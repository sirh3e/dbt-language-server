//! "Split into an incremental model and a table" (`refactor.extract`):
//! the split plan of `features::plan::split_plan` as one WorkspaceEdit —
//! the new incremental model created beside this one, and this one
//! reading it. Pure over the plan.

use std::path::Path;

use cogitator_lsp::{
    CodeAction, CreateFile, DocumentChange, Position, Range, TextDocumentEdit, TextEdit,
    WorkspaceEdit,
};

use super::super::file_uri;
use super::super::plan::SplitPlan;

pub fn split_action(plan: &SplitPlan, uri: &str) -> Option<CodeAction> {
    let part = plan.part.as_ref()?;
    let path = cogitator_lsp::uri_to_fs_path(uri)?;
    let new_uri = file_uri(
        &Path::new(&path)
            .parent()?
            .join(format!("{}.sql", part.name)),
    );
    Some(CodeAction {
        title: format!(
            "Split '{}' into the incremental model '{}' and a table over it",
            part.cte, part.name
        ),
        kind: Some("refactor.extract"),
        diagnostics: None,
        edit: Some(WorkspaceEdit {
            document_changes: vec![
                DocumentChange::Create(CreateFile::new(new_uri.clone())),
                DocumentChange::Edit(TextDocumentEdit::new(
                    new_uri,
                    vec![TextEdit {
                        range: Range::zero_width(Position {
                            line: 0,
                            character: 0,
                        }),
                        new_text: part.text.clone(),
                    }],
                )),
                DocumentChange::Edit(TextDocumentEdit::new(uri.to_string(), plan.edits.clone())),
            ],
        }),
        command: None,
        is_preferred: None,
    })
}
