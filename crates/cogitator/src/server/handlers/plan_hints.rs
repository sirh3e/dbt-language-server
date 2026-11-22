//! Plan inlay hints: the CTE row counts catalog statistics prove
//! (`features::plan::row_inlay_hints`). The shell's part is the gating —
//! the toggle, `perf.catalog_stats`, and a catalog that has any row
//! count at all — so a project without statistics never plans for this.

use cogitator_analysis::artifacts::catalog::CatalogIndex;
use cogitator_analysis::features::plan::diagnostics_config::PerfSettings;
use cogitator_analysis::features::plan::row_inlay_hints;
use cogitator_lsp::{InlayHint, Range};

use super::super::derived_cache::DerivedCache;
use super::super::snapshot::Snapshot;

/// The row-count hints for the open document `uri` within `range`.
pub fn row_hints(
    snap: &Snapshot,
    derived: &DerivedCache,
    uri: &str,
    range: Range,
    perf: &PerfSettings,
    artifact_cap: u64,
) -> Vec<InlayHint> {
    if !perf.row_hints || !perf.catalog_stats {
        return Vec::new();
    }
    let (Some(doc), Some(root)) = (snap.doc.as_deref(), snap.ctx.project_root.as_deref()) else {
        return Vec::new();
    };
    let artifacts = derived.artifacts(root, artifact_cap);
    if !artifacts.catalog.as_deref().is_some_and(has_row_counts) {
        return Vec::new();
    }
    super::plan::document_plan(snap, derived, uri, artifact_cap, perf.catalog_stats)
        .map(|plan| row_inlay_hints(&doc.text, &plan.plan, range))
        .unwrap_or_default()
}

fn has_row_counts(catalog: &CatalogIndex) -> bool {
    catalog
        .models
        .values()
        .chain(catalog.sources.values())
        .any(|table| table.stats.row_count.is_some())
}
