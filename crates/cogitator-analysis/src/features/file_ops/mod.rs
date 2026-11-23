//! The workspace file operations (LSP 3.16 `workspace.fileOperations`):
//! what renaming a model file means to the project (every `ref('old')`
//! and the properties-YAML `name:`, the client renames the file itself)
//! and what deleting one leaves dangling. Pure over the context; file
//! text comes through a `read` closure.

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cogitator_lsp::{DocumentChange, TextDocumentEdit, WorkspaceEdit};

use super::references::files::path_of;
use super::rename::model_ref_edits;
use super::DbtContext;
use crate::context::ModelKind;

/// `workspace/willRenameFiles`: one model rename per renamed model file
/// whose new stem is another valid model name, merged per file. A move
/// that keeps the name, a macro or YAML file, a folder and a snapshot
/// (named by its block, not its file) change nothing dbt sees.
pub fn will_rename_files(
    ctx: &DbtContext,
    renames: &[(String, String)],
    read: &dyn Fn(&Path) -> Option<String>,
) -> WorkspaceEdit {
    let mut by_uri: BTreeMap<String, TextDocumentEdit> = BTreeMap::new();
    for (old, new) in renames {
        let Some((model, new_name)) = renamed_model(ctx, old, new) else {
            continue;
        };
        for change in model_ref_edits(ctx, &model, &new_name, read) {
            let DocumentChange::Edit(edit) = change else {
                continue;
            };
            by_uri
                .entry(edit.text_document.uri.clone())
                .and_modify(|held| held.edits.extend(edit.edits.iter().cloned()))
                .or_insert(edit);
        }
    }
    let document_changes = by_uri
        .into_values()
        .map(|mut edit| {
            edit.edits
                .sort_by_key(|e| (e.range.start.line, e.range.start.character));
            edit.edits.dedup_by_key(|e| e.range);
            DocumentChange::Edit(edit)
        })
        .collect();
    WorkspaceEdit { document_changes }
}

/// `(old model name, new model name)` when renaming `old` to `new`
/// renames a model (or seed) dbt knows by its file stem.
fn renamed_model(ctx: &DbtContext, old: &str, new: &str) -> Option<(String, String)> {
    let (old, new) = (path_of(old)?, path_of(new)?);
    let (model, details) = model_of_file(ctx, &old)?;
    if !matches!(details.kind, ModelKind::Model | ModelKind::Seed) {
        return None;
    }
    if new.extension() != old.extension() {
        return None;
    }
    let new_name = new.file_stem()?.to_str()?.to_string();
    let valid = !new_name.is_empty() && new_name.chars().all(|c| c.is_alphanumeric() || c == '_');
    (valid && new_name != model).then_some((model, new_name))
}

/// The model whose file is `path`, known by its file stem.
fn model_of_file<'a>(
    ctx: &'a DbtContext,
    path: &Path,
) -> Option<(String, &'a crate::context::ModelDetails)> {
    let stem = path.file_stem()?.to_str()?;
    let details = ctx.model_detail_map.get(stem)?;
    (details.uri == path).then(|| (stem.to_string(), details))
}

/// A deleted model and the dependents its deletion leaves with a
/// dangling `ref()`, by name, sorted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletedModel {
    pub model: String,
    pub dependents: Vec<String>,
}

/// `workspace/willDeleteFiles`: the models among the deleted files (a
/// deleted folder deletes every model below it) that something still
/// depends on. A dependent deleted in the same request does not count.
pub fn deleted_with_dependents(ctx: &DbtContext, deleted: &[String]) -> Vec<DeletedModel> {
    let gone: Vec<PathBuf> = deleted.iter().filter_map(|u| path_of(u)).collect();
    let is_gone = |p: &Path| !p.as_os_str().is_empty() && gone.iter().any(|g| p.starts_with(g));
    let mut out: Vec<DeletedModel> = ctx
        .model_detail_map
        .iter()
        .filter(|(_, d)| is_gone(&d.uri))
        .filter_map(|(model, _)| {
            let mut dependents: Vec<String> = ctx
                .graph
                .downstream_of(model)
                .filter(|n| !is_gone(&n.uri))
                .map(|n| n.name.clone())
                .collect();
            dependents.sort();
            dependents.dedup();
            (!dependents.is_empty()).then(|| DeletedModel {
                model: model.clone(),
                dependents,
            })
        })
        .collect();
    out.sort_by(|a, b| a.model.cmp(&b.model));
    out
}

/// The one warning a delete request shows, or none.
pub fn delete_warning(models: &[DeletedModel]) -> Option<String> {
    if models.is_empty() {
        return None;
    }
    let parts: Vec<String> = models
        .iter()
        .map(|m| {
            format!(
                "`{}` is still ref()'d by {}",
                m.model,
                m.dependents.join(", ")
            )
        })
        .collect();
    Some(format!(
        "Deleting {}: {}. Those refs will dangle.",
        if models.len() == 1 {
            "a model with dependents"
        } else {
            "models with dependents"
        },
        parts.join("; ")
    ))
}

/// Does a created, renamed or deleted `uri` change what the context
/// knows? A dbt resource file, or a folder (no extension), does.
pub fn touches_project(uri: &str) -> bool {
    path_of(uri).is_some_and(|p| {
        p.extension()
            .and_then(|e| e.to_str())
            .is_none_or(|e| cogitator_lsp::RESOURCE_EXTENSIONS.contains(&e))
    })
}
