//! Task wiring (specs/rust-port/05 §6.2):
//! reader → dispatcher (owns Documents) → writer; debounced refresh task
//! publishing `Arc<DbtContext>` snapshots over a watch channel.

mod commands;
mod diagnostics;
mod dispatcher;
mod fusion;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use cogitator_analysis::{ContextBuilder, DbtContext};
use cogitator_lsp::rpc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, watch};

pub async fn run(fusion_path: String, use_tree_sitter: bool) -> Result<()> {
    // Backend selection (06 §3.5): tree-sitter is the default; phase-1 stays
    // compiled as the fallback and behind --no-tree-sitter.
    let documents = if use_tree_sitter {
        match cogitator_treesitter::TreeSitterParser::new() {
            Ok(parser) => dbt_ls_documents(Box::new(parser)),
            Err(err) => {
                tracing::warn!(%err, "tree-sitter unavailable; using phase-1 pipeline");
                dbt_ls_documents(Box::new(cogitator_parser::JinjaSqlParser))
            }
        }
    } else {
        dbt_ls_documents(Box::new(cogitator_parser::JinjaSqlParser))
    };

    // writer task: sole owner of stdout (05 §6.2 / 01 §4)
    let (write_tx, mut write_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let request_ids = Arc::new(std::sync::atomic::AtomicI64::new(1));

    let fusion_enabled = Arc::new(AtomicBool::new(false));
    if !fusion_path.is_empty() {
        let flag = fusion_enabled.clone();
        let path = fusion_path.clone();
        let link = commands::ClientLink::new(write_tx.clone(), request_ids.clone());
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
    let refresh_link = commands::ClientLink::new(write_tx.clone(), request_ids.clone());
    tokio::spawn(async move {
        let mut warned: Option<String> = None;
        while let Some(mut root) = refresh_rx.recv().await {
            // debounce: coalesce triggers arriving within 200 ms (05 §6.2)
            while let Ok(Some(next)) =
                tokio::time::timeout(Duration::from_millis(200), refresh_rx.recv()).await
            {
                root = next;
            }
            let built = tokio::task::spawn_blocking({
                let root = root.clone();
                let builder = builder.clone();
                move || {
                    let span = tracing::info_span!("context_refresh", root = %root);
                    let _enter = span.enter();
                    let mut builder = builder.lock().expect("context builder lock");
                    let ctx = builder.build(&root);
                    let (hits, misses) = builder.cache_stats();
                    tracing::debug!(hits, misses, "scan cache totals");
                    ctx
                }
            })
            .await;
            match built {
                Ok(ctx) => {
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
                        Some(format!(
                            "SQL dialect not resolved for profile \
                             '{}'.\n\
                             Reason: profiles.yml was not found or has no \
                             matching target (looked in <workspace>/.dbt/).\n\
                             Solution: create profiles.yml with this profile, \
                             or set DBT_PROFILES_DIR when running without a \
                             workspace root. Function docs and dialect \
                             keywords are generic until resolved.",
                            ctx.project.profile
                        ))
                    } else {
                        None
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
                Err(err) => tracing::warn!(%err, "context refresh panicked"),
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
    Ok(())
}

fn dbt_ls_documents(
    parser: Box<dyn cogitator_parser::DocumentParser + Send>,
) -> cogitator_analysis::Documents {
    cogitator_analysis::Documents::with_parser(parser)
}
