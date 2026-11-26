//! Breaking-change preview (FEATURE_REQUESTS): a model's CURRENT output
//! schema — the columns its SQL produces, from typed lineage — against
//! its last BUILT schema (`target/catalog.json`, else the documented
//! `target/manifest.json` columns), and what that breaks downstream.
//! Pure: the shell reads the artifacts and computes the lineages.
//!
//! Types are optional on both sides. The current side's types come
//! through a caller-supplied hook (`current_columns`'s `type_of`), so
//! inferred output types can plug in without touching this module; a
//! type change is reported only when both types are known.

mod anchor;
mod baseline;
mod diff;
mod downstream;
mod surface;
mod types;

use std::collections::HashMap;

use serde::Serialize;

use super::DbtContext;
use crate::lineage::ModelLineage;

pub use anchor::{Anchors, ItemAnchor};
pub use baseline::{baseline, saved_baseline, Baseline, BaselineColumn, BaselineSource};
pub use diff::{diff, RemovedColumn, SchemaDiff, TypeChange};
pub use downstream::{ChangeKind, ColumnImpact, DownstreamUse, Extras, Related};
pub use surface::{diagnostics, lens, BREAKING_CODE};

/// One column the model produces now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentColumn {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_type: Option<String>,
}

/// The current columns of a model from its lineage; `None` while an
/// unexpanded `*` hides what the model produces (never guess a diff).
/// `type_of` is the type hook: the inferred type of an output column,
/// or `None` (names-only comparison) — pass `&|_| None` until the
/// type-check API is available.
pub fn current_columns(
    lineage: &ModelLineage,
    type_of: &dyn Fn(&str) -> Option<String>,
) -> Option<Vec<CurrentColumn>> {
    if lineage.columns.iter().any(|c| c.name.contains('*')) {
        return None;
    }
    Some(
        lineage
            .columns
            .iter()
            .map(|c| CurrentColumn {
                name: c.name.clone(),
                data_type: type_of(&c.name),
            })
            .collect(),
    )
}

/// Why a report carries no diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    /// Compared against a baseline.
    Compared,
    /// No `target/catalog.json` or `target/manifest.json` entry with
    /// columns for this model: nothing to compare, so silence.
    NoBaseline,
    /// The model's columns are not known (unparseable SQL, an
    /// unexpanded `*`, Jinja control flow without compiled SQL).
    UnknownColumns,
}

/// The `dbt.breakingChanges` answer.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BreakingReport {
    pub model: String,
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline: Option<BaselineInfo>,
    /// "2 breaking change(s): first_name removed, …" — the one-liner.
    pub summary: String,
    pub diff: SchemaDiff,
    /// One entry per removed or retyped column, with what reads it.
    pub impacts: Vec<ColumnImpact>,
    /// Exposures/metrics/semantic models depending on the model.
    pub exposures: Vec<String>,
    /// `access: public` on the model itself: consumers outside the
    /// project may read any column.
    pub public: bool,
    /// The downstream lineage was capped (`IMPACT_LINEAGE_CAP`).
    pub truncated: bool,
    /// The open buffer against the file as last saved: what THIS edit
    /// breaks (absent while the buffer is the saved file).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since_save: Option<Box<BreakingReport>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BaselineInfo {
    pub source: BaselineSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<String>,
}

impl BreakingReport {
    /// Removed plus retyped columns — what the lens counts.
    pub fn breaking(&self) -> usize {
        self.diff.removed.len() + self.diff.retyped.len()
    }

    /// A report that compares nothing.
    pub fn without(model: &str, status: Status, baseline: Option<&Baseline>) -> Self {
        let summary = match status {
            Status::NoBaseline => "no built schema in target/ to compare with".to_string(),
            Status::UnknownColumns => "the model's current columns are not known".to_string(),
            Status::Compared => "no breaking changes".to_string(),
        };
        BreakingReport {
            model: model.to_string(),
            status,
            baseline: baseline.map(Baseline::info),
            summary,
            diff: SchemaDiff::default(),
            impacts: Vec::new(),
            exposures: Vec::new(),
            public: false,
            truncated: false,
            since_save: None,
        }
    }

    /// The report with what this edit breaks attached; an edit that
    /// breaks nothing attaches nothing.
    pub fn with_since_save(mut self, edit: Option<BreakingReport>) -> Self {
        self.since_save = edit
            .filter(|e| e.status == Status::Compared && e.breaking() > 0)
            .map(Box::new);
        self
    }
}

/// The whole preview. `lineages` are the downstream models' typed
/// lineages (`impact_targets`), as `dbt.impact` takes them.
pub fn report(
    ctx: &DbtContext,
    model: &str,
    baseline: &Baseline,
    current: &[CurrentColumn],
    lineages: &HashMap<String, ModelLineage>,
) -> BreakingReport {
    report_with(ctx, model, baseline, current, lineages, &Extras::default())
}

/// [`report`] with the model's SQL anchors and the properties corpus
/// (semantic models and metrics at column level).
pub fn report_with(
    ctx: &DbtContext,
    model: &str,
    baseline: &Baseline,
    current: &[CurrentColumn],
    lineages: &HashMap<String, ModelLineage>,
    extras: &Extras<'_>,
) -> BreakingReport {
    let diff = diff(current, &baseline.columns);
    let closure = super::impact(ctx, model, None, lineages);
    let impacts = downstream::impacts(ctx, model, &diff, lineages, extras);
    let public = ctx
        .model_detail_map
        .get(model)
        .is_some_and(|d| d.access.as_deref() == Some("public"));
    let mut report = BreakingReport {
        model: model.to_string(),
        status: Status::Compared,
        baseline: Some(baseline.info()),
        summary: String::new(),
        diff,
        impacts,
        exposures: closure.exposures,
        public,
        truncated: closure.truncated,
        since_save: None,
    };
    report.summary = surface::summary(&report);
    report
}

#[cfg(test)]
mod tests;
