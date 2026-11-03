//! The debounced context-refresh task (05 §6.2): coalesces triggers,
//! rebuilds the DbtContext off the async workers (scan-cache warm
//! start, optional warehouse enrichment, snapshot persistence), then
//! publishes over the watch channel with one Guidance warning per
//! distinct degradation cause.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use cogitator_analysis::{ContextBuilder, DbtContext};
use tokio::sync::{mpsc, watch};

use super::client_link::ClientLink;
use super::derived_cache::DerivedCache;
use super::{cache_persist, derived_cache, ProfilesConfig, SharedLimits, StatusInfo};

pub(super) struct RefreshTask {
    pub link: ClientLink,
    pub profiles: Arc<std::sync::Mutex<ProfilesConfig>>,
    pub derived: Arc<DerivedCache>,
    pub limits: Arc<SharedLimits>,
    #[cfg(feature = "duckdb-parser")]
    pub toggles: Arc<super::SharedToggles>,
    pub status: Arc<std::sync::Mutex<StatusInfo>>,
    pub revision: Arc<AtomicU64>,
    pub builder: Arc<std::sync::Mutex<ContextBuilder>>,
    pub ctx_tx: watch::Sender<Arc<DbtContext>>,
}

impl RefreshTask {
    pub async fn run(self, mut refresh_rx: mpsc::UnboundedReceiver<String>) {
        let refresh_link = self.link;
        let refresh_profiles = self.profiles;
        let refresh_derived = self.derived;
        let refresh_limits = self.limits;
        #[cfg(feature = "duckdb-parser")]
        let refresh_toggles = self.toggles;
        let refresh_status = self.status;
        let refresh_revision = self.revision;
        let builder = self.builder;
        let ctx_tx = self.ctx_tx;
        let mut warned: Option<String> = None;
        // scan-cache totals are cumulative across builds: keep the last
        // observation so the metrics see per-refresh deltas
        let (mut prev_hits, mut prev_misses) = (0u64, 0u64);
        while let Some(mut root) = refresh_rx.recv().await {
            // debounce: coalesce triggers within the (hot-tunable) window
            let debounce = Duration::from_millis(
                refresh_limits
                    .refresh_debounce_ms
                    .load(std::sync::atomic::Ordering::Relaxed),
            );
            while let Ok(Some(next)) = tokio::time::timeout(debounce, refresh_rx.recv()).await {
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
                #[cfg(feature = "duckdb-parser")]
                let warehouse = (refresh_toggles.clone(), refresh_derived.clone());
                move || {
                    let span = tracing::info_span!("context_refresh", root = %root);
                    let _enter = span.enter();
                    let mut builder = builder.lock().expect("context builder lock");
                    builder.set_profiles_dir(cfg.dir_override.clone());
                    builder.set_profile_override(cfg.profile.clone());
                    builder.set_target_override(cfg.target.clone());
                    // cold process + persisted snapshot → warm start
                    if builder.scan_is_cold() {
                        if let Some(project_root) = cogitator_analysis::util::get_project_root(
                            "dbt_project.yml",
                            std::path::Path::new(&root),
                        ) {
                            if let Some(scan) = server_cache_load(&project_root) {
                                builder.seed_scan(scan);
                            }
                        }
                    }
                    #[allow(unused_mut)]
                    let mut ctx = builder.build(&root);
                    let stats = builder.cache_stats();
                    tracing::debug!(hits = stats.0, misses = stats.1, "scan cache totals");
                    #[cfg(feature = "duckdb-parser")]
                    if warehouse.0.warehouse_columns.load(Ordering::Relaxed) {
                        enrich_warehouse_columns(&mut ctx, &root, &cfg, &warehouse.1);
                    }
                    if let Some(project_root) = ctx.project_root.as_deref() {
                        cache_persist::save(project_root, builder.export_scan());
                    }
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
                    crate::metrics::record_refresh(
                        elapsed_ms as f64,
                        hits.saturating_sub(prev_hits),
                        misses.saturating_sub(prev_misses),
                    );
                    (prev_hits, prev_misses) = (hits, misses);
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
                        Some(
                            cogitator_error::guidance::Guidance::new("No dbt project found.")
                                .because(format!(
                                    "dbt_project.yml was not located from '{root}' \
                                     (searched upward and downward)."
                                ))
                                .fix(
                                    "open the folder containing dbt_project.yml; \
                                     hover/completion for refs stays empty until then.",
                                )
                                .to_message(),
                        )
                    } else if ctx.project_root.is_some()
                        && ctx.dialect.as_str().is_empty()
                        && !ctx.project.profile.is_empty()
                    {
                        let searched: Vec<String> =
                            cogitator_analysis::util::profiles_candidates(&cfg.dir_override, &root)
                                .iter()
                                .map(|p| p.display().to_string())
                                .collect();
                        Some(
                            cogitator_error::guidance::Guidance::new(format!(
                                "SQL dialect not resolved for profile '{}'.",
                                ctx.project.profile
                            ))
                            .because(format!(
                                "no profiles.yml with a matching target was found. \
                                 Searched: {}.",
                                searched.join(", ")
                            ))
                            .fix(
                                "create profiles.yml with this profile in one of those \
                                 places, or point --profiles-dir, \
                                 initializationOptions.profilesDir or DBT_PROFILES_DIR \
                                 at its directory. Function docs and dialect keywords \
                                 are generic until resolved.",
                            )
                            .to_message(),
                        )
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
                    refresh_revision.fetch_add(1, Ordering::Relaxed);
                    let _ = ctx_tx.send(Arc::new(ctx));
                }
                Err(err) => {
                    progress.end("refresh failed");
                    tracing::warn!(%err, "context refresh panicked");
                }
            }
        }
    }
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
    Some(
        cogitator_error::guidance::Guidance::new(format!(
            "Connection auth for {profile_name}/{target_name} ({}, {}) has problems:\n{}",
            target.adapter_type,
            target.auth.method.as_str(),
            list.join("\n")
        ))
        .at(inventory.path.display().to_string())
        .fix("fix the profile — dbt run/test/compile will fail until then.")
        .to_message(),
    )
}

/// duckdb builds, `warehouse.columns: true` only (03 §10): source
/// tables whose YAML declares no columns get them from the warehouse's
/// information_schema, read-only, during the context refresh — never
/// on a request path. Any open/query failure switches the pass off for
/// this refresh with a trace.
#[cfg(feature = "duckdb-parser")]
fn enrich_warehouse_columns(
    ctx: &mut DbtContext,
    root: &str,
    cfg: &ProfilesConfig,
    derived: &derived_cache::DerivedCache,
) {
    if ctx.dialect.as_str() != "duckdb" || ctx.project.profile.is_empty() {
        return;
    }
    let Some(path) = cogitator_analysis::util::profiles_candidates(&cfg.dir_override, root)
        .into_iter()
        .find(|p| p.is_file())
    else {
        return;
    };
    let inventory = derived.profiles_inventory(&path, &ctx.project.profile, &cfg.allow);
    let profile_name = if cfg.profile.is_empty() {
        ctx.project.profile.as_str()
    } else {
        cfg.profile.as_str()
    };
    let Some(profile) = inventory.profile(profile_name) else {
        return;
    };
    let target_name = if cfg.target.is_empty() {
        profile.default_target.as_str()
    } else {
        cfg.target.as_str()
    };
    let Some(target) = profile.targets.iter().find(|t| t.name == target_name) else {
        return;
    };
    let Some(warehouse) = &target.warehouse_path else {
        return;
    };
    let mut warehouse = std::path::PathBuf::from(warehouse);
    if warehouse.is_relative() {
        if let Some(project_root) = ctx.project_root.as_deref() {
            warehouse = project_root.join(warehouse);
        }
    }
    if !warehouse.is_file() {
        return;
    }
    for source in ctx.source_detail_map.values_mut() {
        let schema = source.schema.clone().unwrap_or_else(|| source.name.clone());
        for table in source.tables.values_mut() {
            if !table.columns.is_empty() {
                continue;
            }
            let identifier = table
                .identifier
                .clone()
                .unwrap_or_else(|| table.name.clone());
            match cogitator_duckdb::catalog::table_columns(&warehouse, &schema, &identifier) {
                Ok(columns) => {
                    table.columns = columns
                        .into_iter()
                        .map(|c| cogitator_schemas::ColumnEntry {
                            name: c.name,
                            description: format!("{} (from warehouse)", c.data_type),
                            name_pos: Default::default(),
                        })
                        .collect();
                }
                Err(guidance) => {
                    tracing::debug!(
                        error = %guidance.to_message(),
                        "warehouse introspection off for this refresh"
                    );
                    return;
                }
            }
        }
    }
}

/// Thin indirection so the refresh closure reads cleanly.
fn server_cache_load(
    project_root: &std::path::Path,
) -> Option<cogitator_analysis::context::persist::PersistedScan> {
    cache_persist::load(project_root)
}
