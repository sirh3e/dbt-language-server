//! Plan inlay hints: the CTE row counts catalog statistics prove
//! (`features::plan::row_inlay_hints`) and, with `perf.inline_findings`,
//! the reported findings (`features::plan::finding_hints`), merged where
//! they share a position. The shell's part is the gating — the toggles,
//! `perf.catalog_stats`, a catalog with any row count, an active profile —
//! so a document that wants neither never plans for this, and one that
//! wants both plans once (the cached `document_plan`).

use cogitator_analysis::artifacts::catalog::CatalogIndex;
use cogitator_analysis::features::plan::diagnostics_config::PerfSettings;
use cogitator_analysis::features::plan::{finding_hints, merge_plan_hints, row_inlay_hints};
use cogitator_lsp::{AnyInlayHint, Range};

use super::super::derived_cache::DerivedCache;
use super::super::snapshot::Snapshot;

/// The plan's hints for the open document `uri` within `range`.
pub fn plan_hints(
    snap: &Snapshot,
    derived: &DerivedCache,
    uri: &str,
    range: Range,
    perf: &PerfSettings,
    artifact_cap: u64,
) -> Vec<AnyInlayHint> {
    let (Some(doc), Some(root)) = (snap.doc.as_deref(), snap.ctx.project_root.as_deref()) else {
        return Vec::new();
    };
    let findings = perf.inline_findings && perf.active();
    let rows = perf.row_hints
        && perf.catalog_stats
        && derived
            .artifacts(root, artifact_cap)
            .catalog
            .as_deref()
            .is_some_and(has_row_counts);
    if !rows && !findings {
        return Vec::new();
    }
    let Some(plan) =
        super::plan::document_plan(snap, derived, uri, artifact_cap, perf.catalog_stats)
    else {
        return Vec::new();
    };
    let row_hints = if rows {
        row_inlay_hints(&doc.text, &plan.plan, range)
    } else {
        Vec::new()
    };
    let finding_hints = if findings {
        finding_hints(&doc.text, &plan.plan, perf, range)
    } else {
        Vec::new()
    };
    merge_plan_hints(row_hints, finding_hints, uri)
}

fn has_row_counts(catalog: &CatalogIndex) -> bool {
    catalog
        .models
        .values()
        .chain(catalog.sources.values())
        .any(|table| table.stats.row_count.is_some())
}
