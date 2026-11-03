//! Task wiring (specs/rust-port/05 §6.2):
//! reader → dispatcher (owns Documents) → writer; debounced refresh task
//! publishing `Arc<DbtContext>` snapshots over a watch channel.

mod client_link;
mod commands;
mod compiled;
mod derived_cache;
mod diagnostics;
mod dispatcher;
mod fusion;
mod relation;
mod status;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use cogitator_analysis::{ContextBuilder, DbtContext};
use cogitator_lsp::rpc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, watch};

/// What the server has been doing — read by `dbt.status`, written by the
/// startup wiring and the refresh task.
#[derive(Debug, Default, Clone)]
pub struct StatusInfo {
    pub backend: &'static str,
    pub refreshes: u64,
    pub last_refresh_ms: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
}

/// Shared profiles configuration: seeded from launch flags, replaced by
/// initializationOptions and `dbt.setTarget`, read by every context refresh.
#[derive(Debug, Default, Clone)]
pub struct ProfilesConfig {
    /// `--profiles-dir` / initializationOptions.profilesDir.
    pub dir_override: String,
    /// Profiles readable besides the project's own (`--profiles-allow`).
    pub allow: Vec<String>,
    /// Active profile override; empty → the project's profile.
    pub profile: String,
    /// Active target override; empty → the profile's default target.
    pub target: String,
}

/// Auth problems of the *active* target, as one Reason/Solution warning.
fn auth_warning(
    cfg: &ProfilesConfig,
    root: &str,
    ctx: &DbtContext,
    derived: &derived_cache::DerivedCache,
) -> Option<String> {
    if ctx.project.profile.is_empty() {
        return None;
    }
    let path = cogitator_analysis::util::profiles_candidates(&cfg.dir_override, root)
        .into_iter()
        .find(|p| p.is_file())?;
    let inventory = derived.profiles_inventory(&path, &ctx.project.profile, &cfg.allow);
    let profile_name = if cfg.profile.is_empty() {
        ctx.project.profile.as_str()
    } else {
        cfg.profile.as_str()
    };
    let profile = inventory.profile(profile_name)?;
    let target_name = if cfg.target.is_empty() {
        profile.default_target.as_str()
    } else {
        cfg.target.as_str()
    };
    let target = profile.targets.iter().find(|t| t.name == target_name)?;
    if target.auth.issues.is_empty() {
        return None;
    }
    let list: Vec<String> = target
        .auth
        .issues
        .iter()
        .map(|i| format!("• {}", i.message))
        .collect();
    Some(format!(
        "Connection auth for {profile_name}/{target_name} ({}, {}) has problems:\n{}\n\
         Solution: fix {} — dbt run/test/compile will fail until then.",
        target.adapter_type,
        target.auth.method.as_str(),
        list.join("\n"),
        inventory.path.display()
    ))
}

pub async fn run(
    fusion_path: String,
    use_tree_sitter: bool,
    profiles: ProfilesConfig,
) -> Result<()> {
    // Backend selection (06 §3.5): tree-sitter is the default; phase-1 stays
    // compiled as the fallback and behind --no-tree-sitter. The status
    // backend string reports what actually loaded, not what was asked for.
    let (documents, backend) = if use_tree_sitter {
        match cogitator_treesitter::TreeSitterParser::new() {
            Ok(parser) => (dbt_ls_documents(Box::new(parser)), "tree-sitter"),
            Err(err) => {
                tracing::warn!(%err, "tree-sitter unavailable; using phase-1 pipeline");
                (
                    dbt_ls_documents(Box::new(cogitator_parser::JinjaSqlParser)),
                    "minijinja+sqlparser",
                )
            }
        }
    } else {
        (
            dbt_ls_documents(Box::new(cogitator_parser::JinjaSqlParser)),
            "minijinja+sqlparser",
        )
    };

    // writer task: sole owner of stdout (05 §6.2 / 01 §4)
    let (write_tx, mut write_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let request_ids = Arc::new(std::sync::atomic::AtomicI64::new(1));
    // set at initialize from the client's window.workDoneProgress capability
    let progress_supported = Arc::new(AtomicBool::new(false));
    let status = Arc::new(std::sync::Mutex::new(StatusInfo {
        backend,
        ..StatusInfo::default()
    }));

    let fusion_enabled = Arc::new(AtomicBool::new(false));
    if !fusion_path.is_empty() {
        let flag = fusion_enabled.clone();
        let path = fusion_path.clone();
        let link = client_link::ClientLink::new(
            write_tx.clone(),
            request_ids.clone(),
            progress_supported.clone(),
        );
        tokio::spawn(async move {
            let valid = fusion::validate_fusion(&path).await;
            flag.store(valid, Ordering::Relaxed);
            if !valid {
                link.show_message(
                    cogitator_lsp::message_type::ERROR,
                    format!(
                        "dbt Fusion features are disabled.\n\
                         Reason: '{path}' was not found or is not a \
                         dbt-fusion binary (its --version must report \
                         dbt-fusion).\n\
                         Solution: install dbt Fusion, or pass the correct \
                         path via --fusion <path>."
                    ),
                );
            }
        });
    }
    let writer = tokio::spawn(async move {
        let mut stdout = tokio::io::stdout();
        while let Some(frame) = write_rx.recv().await {
            if stdout.write_all(&frame).await.is_err() {
                break;
            }
            let _ = stdout.flush().await;
        }
    });

    // refresh task: debounced context rebuilds → watch snapshots
    let (ctx_tx, ctx_rx) = watch::channel(Arc::new(DbtContext::default()));
    let (refresh_tx, mut refresh_rx) = mpsc::unbounded_channel::<String>();
    let builder = Arc::new(std::sync::Mutex::new(ContextBuilder::default()));
    // launch flags; initializationOptions and dbt.setTarget replace fields
    let profiles = Arc::new(std::sync::Mutex::new(profiles));
    let refresh_profiles = profiles.clone();
    let derived = Arc::new(derived_cache::DerivedCache::default());
    let refresh_derived = derived.clone();
    let refresh_link = client_link::ClientLink::new(
        write_tx.clone(),
        request_ids.clone(),
        progress_supported.clone(),
    );
    let refresh_status = status.clone();
    tokio::spawn(async move {
        let mut warned: Option<String> = None;
        while let Some(mut root) = refresh_rx.recv().await {
            // debounce: coalesce triggers arriving within 200 ms (05 §6.2)
            while let Ok(Some(next)) =
                tokio::time::timeout(Duration::from_millis(200), refresh_rx.recv()).await
            {
                root = next;
            }
            let cfg = refresh_profiles
                .lock()
                .expect("profiles config lock")
                .clone();
            let progress = refresh_link.progress_begin("dbt: refreshing project");
            let started = std::time::Instant::now();
            let built = tokio::task::spawn_blocking({
                let root = root.clone();
                let builder = builder.clone();
                let cfg = cfg.clone();
                move || {
                    let span = tracing::info_span!("context_refresh", root = %root);
                    let _enter = span.enter();
                    let mut builder = builder.lock().expect("context builder lock");
                    builder.set_profiles_dir(cfg.dir_override);
                    builder.set_profile_override(cfg.profile);
                    builder.set_target_override(cfg.target);
                    let ctx = builder.build(&root);
                    let stats = builder.cache_stats();
                    tracing::debug!(hits = stats.0, misses = stats.1, "scan cache totals");
                    (ctx, stats)
                }
            })
            .await;
            match built {
                Ok((ctx, (hits, misses))) => {
                    let elapsed_ms = started.elapsed().as_millis() as u64;
                    {
                        let mut info = refresh_status.lock().expect("status lock");
                        info.refreshes += 1;
                        info.last_refresh_ms = elapsed_ms;
                        info.cache_hits = hits;
                        info.cache_misses = misses;
                    }
                    progress.end(format!(
                        "{} models, {} sources ({} ms)",
                        ctx.model_detail_map.len(),
                        ctx.source_detail_map.len(),
                        elapsed_ms
                    ));
                    tracing::debug!(
                        models = ctx.model_detail_map.len(),
                        sources = ctx.source_detail_map.len(),
                        "context refreshed"
                    );
                    // proactive feedback: explain *why* features are degraded
                    // and what fixes it — at most once per distinct cause
                    let warning = if ctx.project_root.is_none() && !root.is_empty() {
                        Some(format!(
                            "No dbt project found.\n\
                             Reason: dbt_project.yml was not located from \
                             '{root}' (searched upward and downward).\n\
                             Solution: open the folder containing \
                             dbt_project.yml; hover/completion for refs \
                             stays empty until then."
                        ))
                    } else if ctx.project_root.is_some()
                        && ctx.dialect.as_str().is_empty()
                        && !ctx.project.profile.is_empty()
                    {
                        let searched: Vec<String> =
                            cogitator_analysis::util::profiles_candidates(&cfg.dir_override, &root)
                                .iter()
                                .map(|p| p.display().to_string())
                                .collect();
                        Some(format!(
                            "SQL dialect not resolved for profile '{}'.\n\
                             Reason: no profiles.yml with a matching target \
                             was found. Searched: {}.\n\
                             Solution: create profiles.yml with this profile \
                             in one of those places, or point --profiles-dir, \
                             initializationOptions.profilesDir or \
                             DBT_PROFILES_DIR at its directory. Function docs \
                             and dialect keywords are generic until resolved.",
                            ctx.project.profile,
                            searched.join(", ")
                        ))
                    } else {
                        // dialect fine: check the active target's auth next
                        auth_warning(&cfg, &root, &ctx, &refresh_derived)
                    };
                    if warning != warned {
                        if let Some(text) = &warning {
                            refresh_link
                                .show_message(cogitator_lsp::message_type::WARNING, text.clone());
                        }
                        warned = warning;
                    }
                    let _ = ctx_tx.send(Arc::new(ctx));
                }
                Err(err) => {
                    progress.end("refresh failed");
                    tracing::warn!(%err, "context refresh panicked");
                }
            }
        }
    });

    // reader loop feeding the dispatcher (same task: ordering preserved)
    let mut dispatcher = dispatcher::Dispatcher::new(
        documents,
        write_tx,
        ctx_rx,
        refresh_tx,
        fusion_path,
        fusion_enabled,
        profiles,
        progress_supported,
        status,
        // the ONE server→client request-id counter: the refresh/fusion
        // ClientLinks share it, so dispatcher-issued ids cannot collide
        request_ids,
        derived,
    );

    let mut stdin = tokio::io::stdin();
    let mut buf: Vec<u8> = Vec::with_capacity(64 * 1024);
    let mut chunk = [0u8; 16 * 1024];
    loop {
        // drain complete frames before reading more
        loop {
            match rpc::parse_frame(&buf) {
                Ok(Some(frame)) => {
                    let data: Vec<u8> = buf.drain(..frame.total_len).collect();
                    let content =
                        &data[frame.content_start..frame.content_start + frame.content_len];
                    dispatcher.handle(content).await;
                }
                Ok(None) => break,
                Err(err) => {
                    // Malformed stream: Go's scanner stopped; mirror that.
                    tracing::error!(%err, "malformed frame; shutting down reader");
                    drop(dispatcher);
                    writer.await.ok();
                    crate::observability::flush(); // batched spans survive this exit path too
                    return Ok(());
                }
            }
        }
        let n = stdin.read(&mut chunk).await?;
        if n == 0 {
            break; // EOF → clean exit like Go's scanner loop
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    drop(dispatcher);
    writer.await.ok();
    crate::observability::flush(); // stdin EOF used to lose batched spans
    Ok(())
}

fn dbt_ls_documents(
    parser: Box<dyn cogitator_parser::DocumentParser + Send>,
) -> cogitator_analysis::Documents {
    cogitator_analysis::Documents::with_parser(parser)
}
