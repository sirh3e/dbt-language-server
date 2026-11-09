//! `dbt.queryPlan` (specs/ir-pipeline/09): the shell around the pure
//! driver — it reads text and artifacts, keys the cache, and serializes.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use cogitator_analysis::artifacts::ArtifactSet;
use cogitator_analysis::features::model_name_from_uri;
use cogitator_analysis::features::plan::{query_plan as drive_plan, suppress, PlanInput};
use cogitator_analysis::DbtContext;
use cogitator_error::guidance::Guidance;
use cogitator_lsp::error_codes;

use super::super::derived_cache::{CachedPlan, DerivedCache};
use super::super::reply::Reply;
use super::super::snapshot::Snapshot;
use super::navigation::short_uri;

/// The plan for `uri`, cached per (uri, generation, schema revision).
/// A closed file is read from disk and planned per call.
#[tracing::instrument(skip_all, fields(uri = %short_uri(uri)))]
pub fn query_plan(
    snap: &Snapshot,
    derived: &DerivedCache,
    uri: &str,
    artifact_cap: u64,
    catalog_stats: bool,
) -> Reply {
    let Some(model) = model_name_from_uri(uri) else {
        return invalid(
            Guidance::new("dbt.queryPlan needs a model's .sql file.")
                .at(uri)
                .fix("pass the uri of a dbt model."),
        );
    };
    let artifacts = artifacts_for(snap, derived, artifact_cap);
    let revision = schema_revision(&snap.ctx, &artifacts, catalog_stats);
    let plan = match snap.doc.as_deref() {
        Some(doc) => derived.query_plan(uri, doc.generation, revision, || {
            compute(snap, &artifacts, &model, &doc.text, revision, catalog_stats)
        }),
        None => {
            let path = cogitator_lsp::uri_to_fs_path(uri).unwrap_or_else(|| uri.to_string());
            let Ok(text) = std::fs::read_to_string(&path) else {
                return invalid(
                    Guidance::new(format!("Could not read model '{model}'."))
                        .at(path)
                        .fix("open the model in the editor, or check the uri."),
                );
            };
            compute(snap, &artifacts, &model, &text, revision, catalog_stats).map(Arc::new)
        }
    };
    match plan {
        Some(plan) => Reply::raw(&plan.json),
        None => Reply::null(),
    }
}

/// The plan of an OPEN model, computed on a cache miss: the pull
/// diagnostics path, already offloaded. Absent for anything that is not
/// a model the project knows.
pub fn document_plan(
    snap: &Snapshot,
    derived: &DerivedCache,
    uri: &str,
    artifact_cap: u64,
    catalog_stats: bool,
) -> Option<Arc<CachedPlan>> {
    let doc = snap.doc.as_deref()?;
    let model = model_name_from_uri(uri)?;
    if !snap.ctx.model_detail_map.contains_key(&model) {
        return None;
    }
    let artifacts = artifacts_for(snap, derived, artifact_cap);
    let revision = schema_revision(&snap.ctx, &artifacts, catalog_stats);
    derived.query_plan(uri, doc.generation, revision, || {
        compute(snap, &artifacts, &model, &doc.text, revision, catalog_stats)
    })
}

fn artifacts_for(snap: &Snapshot, derived: &DerivedCache, cap: u64) -> ArtifactSet {
    match snap.ctx.project_root.as_deref() {
        Some(root) => derived.artifacts(root, cap),
        None => ArtifactSet::default(),
    }
}

fn compute(
    snap: &Snapshot,
    artifacts: &ArtifactSet,
    model: &str,
    text: &str,
    revision: u64,
    catalog_stats: bool,
) -> Option<CachedPlan> {
    let mut input = PlanInput::from_context(&snap.ctx, text, Some(model), artifacts, revision);
    input.catalog_stats = catalog_stats;
    let mut plan = drive_plan(&input);
    let suppressions = suppress::scan(text, &snap.ctx.dialect);
    suppress::mark(&mut plan, &suppressions, text);
    let json = serde_json::to_string(&plan).ok()?;
    Some(CachedPlan {
        plan,
        suppressions,
        json: Arc::from(json),
    })
}

/// Changes whenever a plan input does: the context snapshot, a re-read
/// manifest or catalog, or `perf.catalog_stats`. Pointer identity, like
/// the results slot; 48 bits so it stays an exact integer in JSON clients
/// that decode to doubles.
pub fn schema_revision(ctx: &Arc<DbtContext>, artifacts: &ArtifactSet, catalog_stats: bool) -> u64 {
    let mut hasher = DefaultHasher::new();
    (Arc::as_ptr(ctx) as usize).hash(&mut hasher);
    artifacts
        .manifest
        .as_ref()
        .map(|m| Arc::as_ptr(m) as usize)
        .hash(&mut hasher);
    artifacts
        .catalog
        .as_ref()
        .map(|c| Arc::as_ptr(c) as usize)
        .hash(&mut hasher);
    catalog_stats.hash(&mut hasher);
    hasher.finish() & 0xFFFF_FFFF_FFFF
}

fn invalid(guidance: Guidance) -> Reply {
    Reply::error(error_codes::INVALID_PARAMS, guidance.to_message())
}
