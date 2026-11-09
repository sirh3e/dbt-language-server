//! `dbt.queryPlan` (specs/ir-pipeline/09): the shell around the pure
//! driver — it reads text and artifacts, keys the cache, and serializes.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use cogitator_analysis::artifacts::ArtifactSet;
use cogitator_analysis::features::model_name_from_uri;
use cogitator_analysis::features::plan::{query_plan as drive_plan, PlanInput};
use cogitator_error::guidance::Guidance;
use cogitator_lsp::error_codes;

use super::super::derived_cache::DerivedCache;
use super::super::reply::Reply;
use super::super::snapshot::Snapshot;
use super::navigation::short_uri;
use super::plan_compiled::CompiledProbe;

/// The plan for `uri`, cached per (uri, generation, schema revision).
/// A closed file is read from disk and planned per call.
#[tracing::instrument(skip_all, fields(uri = %short_uri(uri)))]
pub fn query_plan(snap: &Snapshot, derived: &DerivedCache, uri: &str, artifact_cap: u64) -> Reply {
    let Some(model) = model_name_from_uri(uri) else {
        return invalid(
            Guidance::new("dbt.queryPlan needs a model's .sql file.")
                .at(uri)
                .fix("pass the uri of a dbt model."),
        );
    };
    let artifacts = match snap.ctx.project_root.as_deref() {
        Some(root) => derived.artifacts(root, artifact_cap),
        None => ArtifactSet::default(),
    };
    let revision = schema_revision(snap, &artifacts);
    let compiled = CompiledProbe::new(&snap.ctx, uri);
    let compute = |text: &str| {
        let file = compiled.read(snap.compiled_cap_bytes);
        let input = PlanInput::from_context(&snap.ctx, text, Some(&model), &artifacts, revision)
            .with_compiled(file.input());
        serde_json::to_string(&drive_plan(&input)).ok()
    };
    // A fresh `dbt compile` changes nothing else in the key.
    let key = compiled.cache_key(revision);
    let json: Option<Arc<str>> = match snap.doc.as_deref() {
        Some(doc) => derived.query_plan(uri, doc.generation, key, || compute(&doc.text)),
        None => {
            let path = cogitator_lsp::uri_to_fs_path(uri).unwrap_or_else(|| uri.to_string());
            let Ok(text) = std::fs::read_to_string(&path) else {
                return invalid(
                    Guidance::new(format!("Could not read model '{model}'."))
                        .at(path)
                        .fix("open the model in the editor, or check the uri."),
                );
            };
            compute(&text).map(|s| Arc::from(s.as_str()))
        }
    };
    match json {
        Some(json) => Reply::raw(&json),
        None => Reply::null(),
    }
}

/// Changes whenever a plan input does: the context snapshot, or a re-read
/// manifest or catalog. Pointer identity, like the results slot; 48 bits
/// so it stays an exact integer in JSON clients that decode to doubles.
fn schema_revision(snap: &Snapshot, artifacts: &ArtifactSet) -> u64 {
    let mut hasher = DefaultHasher::new();
    (Arc::as_ptr(&snap.ctx) as usize).hash(&mut hasher);
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
    hasher.finish() & 0xFFFF_FFFF_FFFF
}

fn invalid(guidance: Guidance) -> Reply {
    Reply::error(error_codes::INVALID_PARAMS, guidance.to_message())
}
