//! dbt.select: dbt's node selection over the project, as `dbt ls
//! --select … --exclude …` would answer it, for the editors' finders.

use cogitator_analysis::features::selection;
use serde::Serialize;

use crate::server::derived_cache::DerivedCache;
use crate::server::reply::Reply;
use crate::server::snapshot::Snapshot;

#[derive(Serialize)]
struct Answer {
    #[serde(flatten)]
    result: selection::SelectionResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    vocabulary: Option<selection::Vocabulary>,
}

/// `{uri, select, exclude?, vocabulary?}` → the nodes, their counts, an
/// error with its span, and (when asked) what completion offers.
pub fn select(
    snap: &Snapshot,
    derived: &DerivedCache,
    select: &str,
    exclude: Option<&str>,
    with_vocabulary: bool,
) -> Reply {
    let manifest = snap
        .ctx
        .project_root
        .as_deref()
        .and_then(|root| derived.artifacts_cached(root).manifest);
    let universe = derived.selection_universe(&snap.ctx, manifest.as_ref());
    let result = selection::select(&snap.ctx, &universe, select, exclude);
    let vocabulary = with_vocabulary.then(|| selection::vocabulary(&snap.ctx, &universe));
    Reply::result(&Answer { result, vocabulary })
}
