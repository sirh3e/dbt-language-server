//! Properties-YAML quickfix actions (03 §10): scaffold id-column
//! tests. Inheriting descriptions lives in `inherit_docs`.

use cogitator_lsp::{
    CodeAction, DocumentChange, Position, Range, TextDocumentEdit, TextEdit, WorkspaceEdit,
};

use crate::context::ModelDetails;

use super::super::tests_nav::ModelTest;

/// `data_tests: [unique, not_null]` for id-looking columns that have
/// none — one action covering them all.
pub fn scaffold_tests_action(
    text: &str,
    uri: &str,
    model: &str,
    details: &ModelDetails,
    tests: &[ModelTest],
) -> Option<CodeAction> {
    let tested: Vec<&str> = tests.iter().filter_map(|t| t.column.as_deref()).collect();
    let mut edits = Vec::new();
    for column in &details.columns {
        let is_key = column.name == "id" || column.name.ends_with("_id");
        if !is_key || tested.iter().any(|t| t.eq_ignore_ascii_case(&column.name)) {
            continue;
        }
        let Some(edit) = insert_under_column(
            text,
            column.name_pos,
            "data_tests:\n  - unique\n  - not_null",
        ) else {
            continue;
        };
        edits.push(edit);
    }
    (!edits.is_empty()).then(|| CodeAction {
        title: format!(
            "Add unique + not_null tests for {} key column(s) of '{model}'",
            edits.len()
        ),
        kind: Some("quickfix"),
        diagnostics: None,
        edit: Some(WorkspaceEdit {
            document_changes: vec![DocumentChange::Edit(TextDocumentEdit::new(
                uri.to_string(),
                edits,
            ))],
        }),
        command: None,
        is_preferred: None,
        data: None,
    })
}

/// A TextEdit inserting `body` (indent-adjusted) on the line after the
/// column's `name:` line.
pub(crate) fn insert_under_column(text: &str, name_pos: Position, body: &str) -> Option<TextEdit> {
    let line = text.lines().nth(name_pos.line as usize)?;
    // align with the `name` KEY of the `- name: x` item
    let dash = line.find('-')?;
    let indent = " ".repeat(dash + 2);
    let mut new_text = String::new();
    for piece in body.lines() {
        new_text.push_str(&indent);
        new_text.push_str(piece);
        new_text.push('\n');
    }
    let at = Position {
        line: name_pos.line + 1,
        character: 0,
    };
    Some(TextEdit {
        range: Range { start: at, end: at },
        new_text,
    })
}
