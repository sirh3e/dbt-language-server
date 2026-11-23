//! The shell of the breaking-change preview: reads the target/
//! artifacts and the typed lineages (the model's and, when something
//! changed, its downstream models' — capped like dbt.impact), then
//! hands `features::breaking` values. Offloaded callers only.

use std::collections::HashMap;

use cogitator_analysis::features::{self, breaking};

use super::derived_cache::DerivedCache;
use super::handlers::lineage::TypedLineage;
use super::snapshot::Snapshot;

/// Where the current columns come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The open buffer (the command).
    Buffer,
    /// The file on disk (open/save, so the diagnostic matches the save).
    Disk,
}

/// The type of an output column of the model at `uri`, from the inferred
/// output-column types (`features::column_types`) of the published
/// project schema snapshot, which the saved files typed: only a proven
/// type with an exact spelling in the project's dialect counts, every
/// other column is `None` and is compared by name only. Never runs the
/// schema job.
fn column_type_of<'a>(
    snap: &'a Snapshot,
    derived: &'a DerivedCache,
    uri: &'a str,
) -> impl Fn(&str) -> Option<String> + 'a {
    let types = features::model_name_from_uri(uri).and_then(|model| {
        let root = snap.ctx.project_root.as_deref()?;
        let schemas = derived.project_schemas(root)?;
        features::column_types::column_types(&snap.ctx, &schemas, &model)
    });
    move |column| {
        let column = types.as_ref()?.column(column)?;
        if !column.is_proven() {
            return None;
        }
        features::column_types::render(&column.sql_type, &snap.ctx.dialect)
    }
}

/// The report for the model at `uri`; `None` when `uri` is no model of
/// the project.
pub fn report(
    snap: &Snapshot,
    derived: &DerivedCache,
    uri: &str,
    artifact_cap: u64,
    source: Source,
) -> Option<breaking::BreakingReport> {
    let model = features::model_name_from_uri(uri)?;
    if !snap.ctx.model_detail_map.contains_key(&model) {
        return None;
    }
    let Some(root) = snap.ctx.project_root.as_deref() else {
        return Some(breaking::BreakingReport::without(
            &model,
            breaking::Status::NoBaseline,
            None,
        ));
    };
    let artifacts = derived.artifacts(root, artifact_cap);
    let Some(baseline) = breaking::baseline(&model, &artifacts) else {
        return Some(breaking::BreakingReport::without(
            &model,
            breaking::Status::NoBaseline,
            None,
        ));
    };
    let typed = TypedLineage::new(snap, derived, artifact_cap);
    let lineage = match (source, snap.doc.as_deref()) {
        (Source::Buffer, Some(doc)) => (*typed.for_open_document(doc, uri)).clone(),
        _ => typed.for_saved_file(uri),
    };
    let type_of = column_type_of(snap, derived, uri);
    let Some(current) = lineage
        .as_ref()
        .and_then(|l| breaking::current_columns(l, &type_of))
    else {
        return Some(breaking::BreakingReport::without(
            &model,
            breaking::Status::UnknownColumns,
            Some(&baseline),
        ));
    };
    let changed = breaking::diff(&current, &baseline.columns);
    let mut lineages = HashMap::new();
    if !changed.removed.is_empty() || !changed.retyped.is_empty() {
        for (name, path) in features::impact_targets(&snap.ctx, &model) {
            let uri = cogitator_lsp::fs_path_to_uri(&path);
            if let Some(lineage) = typed.for_saved_file(&uri) {
                lineages.insert(name, lineage);
            }
        }
    }
    Some(breaking::report(
        &snap.ctx, &model, &baseline, &current, &lineages,
    ))
}
