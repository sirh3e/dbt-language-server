//! dbt.syncYaml / `cogitator yaml-sync`: the properties YAML of every
//! model of the root project brought in line with its SQL, what
//! dbt-osmosis's `yaml refactor` does, on the pieces the single-entry
//! actions already use:
//!
//! 1. a model without an entry gets one, where `dbt.extractModelYaml`
//!    would put it (`<model>.yml` beside the SQL) or in the folder's
//!    `_<folder>__models.yml` ([`NewEntries`]);
//! 2. produced columns (typed lineage) the entry lacks are appended, in
//!    the model's output order;
//! 3. a column without a description inherits the one of the column it
//!    passes through unchanged, from the nearest documented ancestor
//!    ([`inherit`]); a written description is never touched. With
//!    `inherit_types`, a missing `data_type:` is filled where the type
//!    inference proves one;
//! 4. documented columns the model no longer produces are reported, and
//!    removed only with `prune`;
//! 5. with `reorder`, the column items follow the SQL's order.
//!
//! Pure: lineage, types and file text come through [`SyncInputs`]. The
//! YAML is edited line by line ([`entry`], `code_actions::yaml_text`) and
//! re-read after every phase; nothing is re-serialized, so what the sync
//! did not touch — comments, quoting, order, blank lines — stays as it
//! was. A model whose lineage is unclear is skipped, with the reason.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use cogitator_lsp::{CreateFile, DocumentChange, TextDocumentEdit, TextEdit, WorkspaceEdit};

use super::column_types::ModelColumnTypes;
use crate::lineage::{ModelLineage, SourceRef};
use crate::DbtContext;

mod create;
mod entry;
mod fill;
mod inherit;
mod order;
mod select;
mod text;

#[cfg(test)]
mod test_project;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_format;

/// Where a model without an entry gets one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NewEntries {
    /// `<model>.yml` beside the model, as `dbt.extractModelYaml` writes it.
    #[default]
    PerModel,
    /// `_<folder>__models.yml` in the model's folder, shared by its models.
    PerFolder,
}

#[derive(Debug, Clone, Default)]
pub struct SyncOptions {
    /// Only these models; empty is every model of the root project.
    pub models: Vec<String>,
    /// Remove documented columns the model no longer produces.
    pub prune: bool,
    /// Fill missing `data_type:` from proven inferred types.
    pub inherit_types: bool,
    /// Put the column items in the SQL's output order.
    pub reorder: bool,
    pub new_entries: NewEntries,
}

/// What the planner reads, all through the shell.
pub struct SyncInputs<'a> {
    pub ctx: &'a DbtContext,
    /// A model's typed column lineage, `None` when it is unclear.
    pub lineage: &'a dyn Fn(&str) -> Option<ModelLineage>,
    /// A model's inferred output-column types (`column_types`).
    pub types: &'a dyn Fn(&str) -> Option<ModelColumnTypes>,
    /// A file's current text (the editor's, when open); `None` when absent.
    pub read: &'a dyn Fn(&Path) -> Option<String>,
}

/// One file the sync changes.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub uri: String,
    /// The file does not exist yet.
    pub created: bool,
    /// Line edits against the file as read (whole content when created).
    pub edits: Vec<TextEdit>,
    /// A unified diff, for previews.
    pub diff: String,
    #[serde(skip)]
    pub path: PathBuf,
    #[serde(skip)]
    pub new_text: String,
}

/// A description written from upstream.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inherited {
    pub column: String,
    /// The documented column it was read from.
    pub from: SourceRef,
    /// The undocumented models in between, nearest first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub via: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Typed {
    pub column: String,
    pub data_type: String,
}

/// Something the sync left alone, and why.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Skip {
    /// `None` when the whole model was skipped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSummary {
    pub model: String,
    /// The properties file of the entry, once known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    pub created_entry: bool,
    pub added: Vec<String>,
    pub inherited: Vec<Inherited>,
    pub typed: Vec<Typed>,
    /// Documented, no longer produced.
    pub stale: Vec<String>,
    pub pruned: Vec<String>,
    pub reordered: bool,
    /// Still without a description after the sync.
    pub undocumented: Vec<String>,
    pub skipped: Vec<Skip>,
    pub changes: usize,
}

impl ModelSummary {
    fn skip(&mut self, column: Option<&str>, reason: impl Into<String>) {
        self.skipped.push(Skip {
            column: column.map(str::to_string),
            reason: reason.into(),
        });
    }

    fn count(&mut self) {
        self.changes = self.added.len()
            + self.inherited.len()
            + self.typed.len()
            + self.pruned.len()
            + usize::from(self.reordered)
            + usize::from(self.created_entry && self.added.is_empty());
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPlan {
    pub files: Vec<FileChange>,
    pub models: Vec<ModelSummary>,
    /// Changes over every model; 0 means the YAML is in sync.
    pub changes: usize,
}

impl SyncPlan {
    /// The plan as one workspace edit: files created, then edited.
    pub fn workspace_edit(&self) -> WorkspaceEdit {
        let mut document_changes = Vec::new();
        for file in &self.files {
            if file.created {
                document_changes.push(DocumentChange::Create(CreateFile::new(file.uri.clone())));
            }
            document_changes.push(DocumentChange::Edit(TextDocumentEdit::new(
                file.uri.clone(),
                file.edits.clone(),
            )));
        }
        WorkspaceEdit { document_changes }
    }
}

/// A properties file as the sync goes: as read, and as it stands now.
struct FileState {
    original: Option<String>,
    current: String,
}

/// The plan for `options` over `inputs`.
pub fn plan_sync(inputs: &SyncInputs<'_>, options: &SyncOptions) -> SyncPlan {
    let mut lineages = inherit::Lineages::new(inputs.lineage);
    let mut files: BTreeMap<PathBuf, FileState> = BTreeMap::new();
    let mut models = Vec::new();
    for (model, details) in select::selected(inputs.ctx, options) {
        let mut summary = ModelSummary {
            model: model.clone(), // boundary: the summary owns its name
            ..ModelSummary::default()
        };
        let Some(details) = details else {
            summary.skip(None, "not a SQL model of the root project");
            models.push(summary);
            continue;
        };
        if let Some(base) = select::version_of(inputs.ctx, &model, details) {
            summary.skip(
                None,
                format!("a version file of '{base}': versions are not synced"),
            );
            models.push(summary);
            continue;
        }
        let path = details
            .schema_uri
            .clone()
            .unwrap_or_else(|| select::new_entry_path(details, &model, options.new_entries));
        let state = files.entry(path.clone()).or_insert_with(|| {
            let original = (inputs.read)(&path);
            FileState {
                current: original.clone().unwrap_or_default(),
                original,
            }
        });
        summary.uri = Some(cogitator_lsp::fs_path_to_uri(&path));
        let work = entry::Work {
            ctx: inputs.ctx,
            model: &model,
            path: &path,
            types: inputs.types,
            options,
        };
        entry::sync_model(&work, &mut lineages, &mut state.current, &mut summary);
        summary.count();
        models.push(summary);
    }
    let files: Vec<FileChange> = files
        .into_iter()
        .filter_map(|(path, state)| file_change(path, state))
        .collect();
    let changes = models.iter().map(|m| m.changes).sum();
    SyncPlan {
        files,
        models,
        changes,
    }
}

fn file_change(path: PathBuf, state: FileState) -> Option<FileChange> {
    let old = state.original.as_deref().unwrap_or("");
    if state.original.is_some() && old == state.current {
        return None;
    }
    if state.original.is_none() && state.current.is_empty() {
        return None;
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let edits = if state.original.is_none() {
        vec![TextEdit {
            range: cogitator_lsp::Range::default(),
            new_text: state.current.clone(), // boundary: the edit owns the text
        }]
    } else {
        text::line_edits(old, &state.current)
    };
    Some(FileChange {
        uri: cogitator_lsp::fs_path_to_uri(&path),
        created: state.original.is_none(),
        diff: text::unified(old, &state.current, &name),
        edits,
        path,
        new_text: state.current,
    })
}
