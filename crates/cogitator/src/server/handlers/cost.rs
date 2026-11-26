//! `dbt.costReport {}`: the last run's models ranked by execution time,
//! with the adapter's bytes and slots (plus an optional query-history
//! export, `cost.query_history`), the build's critical path, and "make
//! it incremental" for slow table models the plan says could be.

use std::path::Path;

use cogitator_analysis::features::cost;

use super::super::config_file::AnalyticsSettings;
use super::super::derived_cache::DerivedCache;
use super::super::reply::Reply;
use super::super::snapshot::Snapshot;

#[tracing::instrument(skip_all)]
pub fn cost_report(
    snap: &Snapshot,
    derived: &DerivedCache,
    artifact_cap: u64,
    settings: &AnalyticsSettings,
) -> Reply {
    let (artifacts, incrementality) = super::plan::incrementality_of(snap, derived, artifact_cap);
    let history = settings
        .query_history
        .as_deref()
        .and_then(|path| read_history(snap.ctx.project_root.as_deref(), path));
    let report = cost::cost_report(
        &snap.ctx,
        &artifacts,
        Some(&incrementality),
        history.as_ref(),
    );
    Reply::result(&report)
}

/// The export, or `None` with a warning in the log: a broken history
/// file degrades the report to run_results alone, it never fails it.
fn read_history(root: Option<&Path>, path: &str) -> Option<cost::QueryHistory> {
    let path = Path::new(path);
    let path = match root {
        Some(root) if path.is_relative() => root.join(path),
        _ => path.to_path_buf(),
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) => {
            tracing::warn!(path = %path.display(), %err, "cost.query_history unreadable; ignored");
            return None;
        }
    };
    match cost::parse_history(&text, &path.display().to_string()) {
        Ok(history) => Some(history),
        Err(guidance) => {
            tracing::warn!(error = %guidance.to_message(), "cost.query_history ignored");
            None
        }
    }
}
