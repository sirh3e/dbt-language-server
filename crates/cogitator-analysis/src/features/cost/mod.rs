//! The cost and performance lens: what the last dbt run cost, per model,
//! from `target/run_results.json` (execution time and the adapter's
//! bytes, slots and rows) plus an optional query-history export.
//! `dbt.costReport` ranks the models, walks the build's critical path
//! and suggests "make it incremental" where the query plan says a slow
//! `table` model could be. Pure: the shell hands in the artifacts, the
//! incrementality report and the parsed history.

mod history;

use serde::Serialize;

pub use history::{parse_history, QueryHistory};

use super::plan::IncrementalityReport;
use crate::artifacts::adapter_response::{human_bytes, AdapterStats};
use crate::artifacts::run_results::RunResults;
use crate::artifacts::status::{run_status_by_model, RunStatus};
use crate::artifacts::ArtifactSet;
use crate::DbtContext;

/// A model counts as slow from here on: the slowest quarter of the run…
const SLOW_SHARE: u32 = 25;
/// …or at least a minute.
const SLOW_SECONDS: f64 = 60.0;
/// The lens names the share only when the run timed this many models.
const MIN_TIMED: usize = 4;

pub const MAKE_INCREMENTAL: &str = "make it incremental";

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CostReport {
    pub generated_at: Option<String>,
    /// Seconds of every model in the run (not wall clock: threads overlap).
    pub total_seconds: f64,
    pub models: Vec<ModelCost>,
    pub critical_path: CriticalPath,
    /// Whether a query-history export added figures.
    pub history_rows: usize,
    /// The report as plain text, for clients that only show it.
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCost {
    pub model: String,
    pub uri: Option<String>,
    pub status: String,
    pub seconds: f64,
    /// 5 = among the slowest 5% of the timed models.
    pub slowest_percent: u32,
    #[serde(flatten)]
    pub adapter: AdapterStats,
    pub materialized: Option<String>,
    pub on_critical_path: bool,
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct CriticalPath {
    pub models: Vec<String>,
    pub seconds: f64,
}

/// `k` models strictly slower than `seconds` among `n` → the share of
/// the run, rounded up, that is at least this slow.
fn slowest_percent(seconds: f64, all: &[f64]) -> u32 {
    let slower = all.iter().filter(|t| **t > seconds).count();
    (((slower + 1) * 100).div_ceil(all.len().max(1))) as u32
}

fn model_times(rr: &RunResults) -> Vec<f64> {
    run_status_by_model(rr)
        .values()
        .map(|s| s.execution_time)
        .collect()
}

/// The cost parts of the "last run" lens: `3.2 GB scanned`, `slowest 5%`.
pub fn lens_parts(rr: &RunResults, _model: &str, status: &RunStatus) -> Vec<String> {
    let mut parts = Vec::new();
    if let Some(bytes) = status.adapter.bytes_scanned() {
        parts.push(format!("{} scanned", human_bytes(bytes)));
    }
    let times = model_times(rr);
    if times.len() >= MIN_TIMED && status.execution_time > 0.0 {
        let share = slowest_percent(status.execution_time, &times);
        if share <= SLOW_SHARE {
            parts.push(format!("slowest {share}%"));
        }
    }
    parts
}

pub fn cost_report(
    ctx: &DbtContext,
    artifacts: &ArtifactSet,
    incrementality: Option<&IncrementalityReport>,
    history: Option<&QueryHistory>,
) -> CostReport {
    let Some(rr) = artifacts.run_results.as_deref() else {
        return CostReport {
            generated_at: None,
            total_seconds: 0.0,
            models: Vec::new(),
            critical_path: CriticalPath::default(),
            history_rows: 0,
            text: "No target/run_results.json: run dbt (build or run) first.\n".to_string(),
        };
    };
    let statuses = run_status_by_model(rr);
    let times: Vec<f64> = statuses.values().map(|s| s.execution_time).collect();
    let seconds_of = |name: &str| statuses.get(name).map(|s| s.execution_time).unwrap_or(0.0);
    let (path, path_seconds) = ctx.graph.critical_path(&seconds_of);
    let mut history_rows = 0;
    let mut models: Vec<ModelCost> = statuses
        .iter()
        .map(|(name, status)| {
            let mut adapter = status.adapter.clone();
            if let Some(found) =
                history.and_then(|h| h.lookup(name, status.adapter.query_id.as_deref()))
            {
                history_rows += 1;
                adapter = adapter.filled_from(found);
            }
            let slowest = slowest_percent(status.execution_time, &times);
            let could = incrementality.is_some_and(|report| {
                report
                    .models
                    .iter()
                    .any(|m| &m.model == name && m.view.summary.is_some())
            });
            let slow = slowest <= SLOW_SHARE || status.execution_time >= SLOW_SECONDS;
            ModelCost {
                model: name.clone(),
                uri: ctx
                    .model_detail_map
                    .get(name)
                    .map(|d| cogitator_lsp::fs_path_to_uri(&d.uri)),
                status: status.status.clone(),
                seconds: status.execution_time,
                slowest_percent: slowest,
                adapter,
                materialized: artifacts
                    .manifest
                    .as_deref()
                    .and_then(|m| m.models.get(name))
                    .and_then(|n| n.config.materialized.clone()),
                on_critical_path: path.contains(name),
                suggestion: (could && slow).then(|| MAKE_INCREMENTAL.to_string()),
            }
        })
        .collect();
    models.sort_by(|a, b| {
        b.seconds
            .total_cmp(&a.seconds)
            .then_with(|| a.model.cmp(&b.model))
    });
    let mut report = CostReport {
        generated_at: rr.metadata.generated_at.clone(),
        total_seconds: times.iter().sum(),
        models,
        critical_path: CriticalPath {
            models: path,
            seconds: path_seconds,
        },
        history_rows,
        text: String::new(),
    };
    report.text = render(&report);
    report
}

fn render(report: &CostReport) -> String {
    let mut out = format!(
        "dbt cost report — last run {} · {} models · {:.1} s of model time\n",
        report.generated_at.as_deref().unwrap_or("(no timestamp)"),
        report.models.len(),
        report.total_seconds
    );
    if !report.critical_path.models.is_empty() {
        out.push_str(&format!(
            "critical path ({:.1} s): {}\n",
            report.critical_path.seconds,
            report.critical_path.models.join(" → ")
        ));
    }
    out.push('\n');
    for m in &report.models {
        let mut line = format!("{:>8.1} s  {:<32}", m.seconds, m.model);
        if let Some(bytes) = m.adapter.bytes_scanned() {
            line.push_str(&format!("  {} scanned", human_bytes(bytes)));
        }
        if let Some(slots) = m.adapter.slot_ms {
            line.push_str(&format!("  {slots} slot ms"));
        }
        if m.on_critical_path {
            line.push_str("  [critical path]");
        }
        if let Some(s) = &m.suggestion {
            line.push_str(&format!("  → {s}"));
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests;
