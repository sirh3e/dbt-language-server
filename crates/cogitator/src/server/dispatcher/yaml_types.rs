//! The shell of the column-type checks: reads the project schema snapshot
//! (never runs the job) and hands the pure checks a lookup over it.
//! Offloaded callers only, except [`hover`], which only reads the
//! published snapshot.

use std::sync::Arc;

use cogitator_analysis::features::column_types::{self, ModelColumnTypes};
use cogitator_analysis::features::plan::schemas::SchemaSnapshot;
use cogitator_analysis::{DbtContext, Document};
use cogitator_lsp::{CodeAction, Diagnostic, Position, Range};

use crate::server::derived_cache::DerivedCache;

pub(crate) fn snapshot(ctx: &DbtContext, derived: &DerivedCache) -> Option<Arc<SchemaSnapshot>> {
    derived.project_schemas(ctx.project_root.as_deref()?)
}

pub(crate) fn lookup<'a>(
    ctx: &'a DbtContext,
    snapshot: &'a SchemaSnapshot,
) -> impl Fn(&str) -> Option<ModelColumnTypes> + 'a {
    move |model| column_types::column_types(ctx, snapshot, model)
}

/// `inlay.types`: the inferred type after each output column of an
/// open model, from the published snapshot (never runs the job).
pub(super) fn inlay_hints(
    ctx: &DbtContext,
    derived: &DerivedCache,
    doc: &Document,
    uri: &str,
    range: Range,
) -> Vec<cogitator_lsp::InlayHint> {
    let Some(model) = cogitator_analysis::features::model_name_from_uri(uri) else {
        return Vec::new();
    };
    let Some(snapshot) = snapshot(ctx, derived) else {
        return Vec::new();
    };
    let Some(types) = column_types::column_types(ctx, &snapshot, &model) else {
        return Vec::new();
    };
    column_types::type_inlay_hints(&doc.parsed, &ctx.dialect, &types, range)
}

/// `column-type-mismatch` and `unit-test-value-type` of a properties file.
pub(super) fn diagnostics(
    ctx: &DbtContext,
    derived: &DerivedCache,
    yaml_text: &str,
) -> Vec<Diagnostic> {
    let Some(snapshot) = snapshot(ctx, derived) else {
        return Vec::new(); // before the first job: nothing is proven yet
    };
    let types = lookup(ctx, &snapshot);
    let mut out = column_types::type_mismatch_diagnostics(ctx, yaml_text, &types);
    out.extend(column_types::unit_test_value_diagnostics(
        yaml_text,
        &ctx.dialect,
        &types,
    ));
    out
}

/// "Fill data_type from the inferred types" at the cursor.
pub(super) fn fill_action(
    ctx: &DbtContext,
    derived: &DerivedCache,
    yaml_text: &str,
    uri: &str,
    range: Range,
) -> Option<CodeAction> {
    let snapshot = snapshot(ctx, derived)?;
    let types = lookup(ctx, &snapshot);
    column_types::fill_data_types_action(ctx, yaml_text, uri, range, &types)
}

/// The type line of a column under the cursor: a YAML column entry, or an
/// output column named in the model's SQL. `None` when nothing is typed.
pub(super) fn hover(
    ctx: &DbtContext,
    derived: &DerivedCache,
    doc: &Document,
    uri: &str,
    pos: Position,
) -> Option<String> {
    let snapshot = snapshot(ctx, derived)?;
    let types = lookup(ctx, &snapshot);
    if crate::server::diagnostics::is_yaml(uri) {
        return column_types::yaml_column_hover(ctx, &doc.text, pos, &types);
    }
    let model = cogitator_analysis::features::model_name_from_uri(uri)?;
    column_types::sql_column_hover(doc, pos, &types(&model)?)
}
