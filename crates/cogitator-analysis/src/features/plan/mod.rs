//! The query-plan feature (specs/ir-pipeline/09): one model's text,
//! driven through HIR, MIR, the passes and LIR, reported level by level.
//!
//! Pure: every fact arrives through [`PlanInput`], the server shell does
//! the filesystem work and caches the JSON.
//!
//! Entry point: [`query_plan`]. Absent facts leave their `Need`
//! unavailable, so rules record `Skipped` rather than guess.

use std::collections::HashMap;

use cogitator_dialects::Dialect;
use cogitator_lower::backends::for_backend;
use cogitator_lower::lower_to_mir;

use crate::artifacts::catalog::CatalogIndex;
use crate::artifacts::manifest::ManifestIndex;
use crate::artifacts::ArtifactSet;
use crate::context::types::SourceDetails;
use crate::{DbtContext, ModelDetails};

mod drive;
mod env;
mod facts;
mod passes;
mod phys;
mod report;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_facts;
mod trace;

pub use report::{
    EvidenceView, FindingView, FixEditView, FixView, LevelReport, OriginView, PlanLevels,
    PlanStatus, QueryPlan,
};
pub use trace::{NeedsView, TraceRow, TraceView};

/// One model's text and the facts known about it.
#[derive(Debug, Clone, Copy)]
pub struct PlanInput<'a> {
    pub text: &'a str,
    pub dialect: &'a Dialect,
    /// The model's name, for `{{ this }}` and its own manifest entry.
    pub model: Option<&'a str>,
    pub models: &'a HashMap<String, ModelDetails>,
    pub sources: &'a HashMap<String, SourceDetails>,
    pub manifest: Option<&'a ManifestIndex>,
    pub catalog: Option<&'a CatalogIndex>,
    /// Stamped into the plan's `ProjectSchemas` and echoed in the report.
    pub schema_revision: u64,
}

impl<'a> PlanInput<'a> {
    /// The input for `text` over a context snapshot and loaded artifacts.
    pub fn from_context(
        ctx: &'a DbtContext,
        text: &'a str,
        model: Option<&'a str>,
        artifacts: &'a ArtifactSet,
        schema_revision: u64,
    ) -> Self {
        PlanInput {
            text,
            dialect: &ctx.dialect,
            model,
            models: &ctx.model_detail_map,
            sources: &ctx.source_detail_map,
            manifest: artifacts.manifest.as_deref(),
            catalog: artifacts.catalog.as_deref(),
            schema_revision,
        }
    }
}

/// Drives `input` through every level. Never panics: gaps become
/// degradations, verifier violations drop their level and everything
/// below it, and a model the template cannot express comes back
/// `Declined` with guidance.
pub fn query_plan(input: &PlanInput<'_>) -> QueryPlan {
    let kind = facts::backend_for(input.dialect);
    // `for_backend` is total today; a future gap degrades to generic
    let Some(backend) =
        for_backend(kind).or_else(|| for_backend(cogitator_ir::lir::Backend::Generic))
    else {
        return QueryPlan::no_backend(input, kind);
    };
    drive::drive(input, backend, &lower_to_mir)
}
