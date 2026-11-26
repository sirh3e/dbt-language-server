//! What completion learns of the model from caches (never a job, never
//! a file): its inferred output columns off the published schema
//! snapshot and its manifest config, for the model-aware snippets and
//! the typed column items.

use cogitator_analysis::features::completion::ModelHints;
use cogitator_analysis::features::{self, column_types};
use cogitator_analysis::DbtContext;

use crate::server::derived_cache::DerivedCache;

pub(super) fn hints(ctx: &DbtContext, derived: &DerivedCache, uri: &str) -> ModelHints {
    let Some(root) = ctx.project_root.as_deref() else {
        return ModelHints::default();
    };
    let snapshot = derived.project_schemas(root);
    let artifacts = derived.artifacts_cached(root);
    let outputs = features::model_name_from_uri(uri).and_then(|model| {
        snapshot
            .as_deref()
            .and_then(|s| column_types::column_types(ctx, s, &model))
    });
    ModelHints {
        outputs,
        snapshot,
        manifest: artifacts.manifest,
        catalog: artifacts.catalog,
    }
}
