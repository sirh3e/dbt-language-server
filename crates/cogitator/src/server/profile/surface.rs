//! What the editor reads of the profiles: the hover section (cache
//! only) and the "Profile column" code action.

use cogitator_analysis::features::{self, profile};
use cogitator_lsp::Position;

use super::super::derived_cache::DerivedCache;
use super::super::snapshot::Snapshot;

/// The hover's profile section: the CACHED profile of the output column
/// under the cursor, current for the last run; never a query.
pub fn hover(
    ctx: &cogitator_analysis::DbtContext,
    derived: &DerivedCache,
    doc: &cogitator_analysis::Document,
    uri: &str,
    pos: Position,
    lineage: Option<&cogitator_analysis::lineage::ModelLineage>,
) -> Option<String> {
    let model = features::model_name_from_uri(uri)?;
    let key = profile::profile_target(ctx, &model, doc, lineage, pos)?;
    let stamp = ctx
        .project_root
        .as_deref()
        .and_then(|root| derived.artifacts_cached(root).run_results)
        .and_then(|rr| rr.metadata.generated_at.clone())
        .unwrap_or_default();
    derived
        .column_profile(&key, &stamp)
        .map(|p| profile::hover_markdown(&p))
}

/// "Profile column 'x'" on an output column (`warehouse.profile` on).
pub fn action(
    snap: &Snapshot,
    derived: &DerivedCache,
    uri: &str,
    pos: Position,
) -> Option<cogitator_lsp::CodeAction> {
    let doc = snap.doc.as_deref()?;
    let model = features::model_name_from_uri(uri)?;
    let lineage = super::super::handlers::commands::lineage_for(derived, &snap.ctx, Some(doc), uri);
    let key = profile::profile_target(&snap.ctx, &model, doc, lineage.as_ref().as_ref(), pos)?;
    Some(profile::profile_action(&key, uri, pos))
}
