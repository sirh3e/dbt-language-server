//! `dbt.incrementality`: every `table` model of the project with its
//! incrementality verdict (specs/ir-pipeline/10), the most expensive
//! rebuilds first — by the last run's execution time where
//! `run_results.json` has one — so the models worth converting lead.
//! Pure over its inputs: the shell hands in each model's text.

use std::path::Path;

use serde::Serialize;

use super::incrementality::IncrementalityView;
use super::schemas::SchemaSnapshot;
use super::{query_plan, PlanInput};
use crate::artifacts::status::run_status_by_model;
use crate::artifacts::ArtifactSet;
use crate::DbtContext;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncrementalityReport {
    pub models: Vec<ModelIncrementality>,
    /// How many table models offer an incremental strategy.
    pub incremental: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelIncrementality {
    pub model: String,
    pub uri: String,
    /// The last run's execution time, in seconds, when known.
    pub last_run_seconds: Option<f64>,
    #[serde(flatten)]
    pub view: IncrementalityView,
}

pub fn incrementality_report(
    ctx: &DbtContext,
    artifacts: &ArtifactSet,
    project: Option<&SchemaSnapshot>,
    schema_revision: u64,
    read: &dyn Fn(&Path) -> Option<String>,
) -> IncrementalityReport {
    let timings = artifacts
        .run_results
        .as_deref()
        .map(run_status_by_model)
        .unwrap_or_default();
    let mut names: Vec<&String> = ctx.model_detail_map.keys().collect();
    names.sort();
    let mut models = Vec::new();
    for name in names {
        let details = &ctx.model_detail_map[name];
        if details.uri.extension().and_then(|e| e.to_str()) != Some("sql") {
            continue;
        }
        let Some(text) = read(&details.uri) else {
            continue;
        };
        let input = PlanInput::from_context(ctx, &text, Some(name), artifacts, schema_revision)
            .with_project(project);
        let Some(view) = query_plan(&input).incrementality else {
            continue; // not a table model
        };
        models.push(ModelIncrementality {
            model: name.clone(),
            uri: cogitator_lsp::fs_path_to_uri(&details.uri),
            last_run_seconds: timings.get(name).map(|t| t.execution_time),
            view,
        });
    }
    // slowest first, then the models with an increment, then by name
    models.sort_by(|a, b| {
        let time = |m: &ModelIncrementality| m.last_run_seconds.unwrap_or(-1.0);
        time(b)
            .total_cmp(&time(a))
            .then_with(|| b.view.summary.is_some().cmp(&a.view.summary.is_some()))
            .then_with(|| a.model.cmp(&b.model))
    });
    let incremental = models.iter().filter(|m| m.view.summary.is_some()).count();
    IncrementalityReport {
        models,
        incremental,
    }
}
