//! What completion learns of the model from caches (never a job, never
//! a file): its inferred output columns off the published schema
//! snapshot and its manifest config, for the model-aware snippets and
//! the typed column items.

use cogitator_analysis::features::completion::ModelHints;
use cogitator_analysis::features::{self, column_types};
use cogitator_analysis::DbtContext;

use crate::server::derived_cache::DerivedCache;

pub(super) fn hints(ctx: &DbtContext, derived: &DerivedCache, uri: &str) -> ModelHints {
    let (Some(root), Some(model)) = (
        ctx.project_root.as_deref(),
        features::model_name_from_uri(uri),
    ) else {
        return ModelHints::default();
    };
    let outputs = derived
        .project_schemas(root)
        .and_then(|snapshot| column_types::column_types(ctx, &snapshot, &model));
    let manifest_event_time = derived
        .artifacts_cached(root)
        .manifest
        .as_deref()
        .and_then(|m| m.models.get(&model))
        .and_then(|node| node.config.event_time.clone());
    ModelHints {
        outputs,
        manifest_event_time,
    }
}
