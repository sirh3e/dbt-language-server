//! Quickfixes assembled from the typed `data` payload that rides the
//! context diagnostics back from the client (01 §6.1-6.2): one action
//! per did-you-mean suggestion, carrying the WorkspaceEdit that applies
//! it. Pure over the wire values — no context, no filesystem.

use cogitator_lsp::{
    CodeAction, Diagnostic, DocumentChange, FixKind, TextDocumentEdit, TextEdit, WorkspaceEdit,
};

/// One quickfix per suggestion on every context diagnostic that carries
/// our data payload; foreign diagnostics (no/alien `data`) contribute
/// nothing.
pub fn quickfixes(uri: &str, diagnostics: &[Diagnostic]) -> Vec<CodeAction> {
    let mut out = Vec::new();
    for diagnostic in diagnostics {
        let Some(data) = &diagnostic.data else {
            continue;
        };
        for suggestion in &data.suggestions {
            let title = match data.fix {
                FixKind::ReplaceToken => format!("Change to '{suggestion}'"),
                FixKind::InsertText => format!("Insert '{suggestion}'"),
            };
            let edit = WorkspaceEdit {
                document_changes: vec![DocumentChange::Edit(TextDocumentEdit::new(
                    uri.to_string(),
                    vec![TextEdit {
                        range: data.token_range,
                        new_text: suggestion.clone(),
                    }],
                ))],
            };
            out.push(CodeAction::quickfix(title, diagnostic.clone(), edit));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use cogitator_lsp::{diagnostic_severity, DiagnosticData, Position, Range};

    #[test]
    fn one_action_per_suggestion_with_the_edit() {
        let range = Range::zero_width(Position {
            line: 3,
            character: 22,
        });
        let mut diag = Diagnostic::plain(
            range,
            "model 'stg_order' not found — did you mean 'stg_orders'?".into(),
            diagnostic_severity::WARNING,
            "dangling-ref",
            "cogitator",
        );
        diag.data = Some(DiagnosticData {
            fix: FixKind::ReplaceToken,
            token_range: range,
            suggestions: vec!["stg_orders".into(), "stg_payments".into()],
        });
        let plain = Diagnostic::plain(
            range,
            "foreign".into(),
            diagnostic_severity::ERROR,
            "x",
            "elsewhere",
        );
        let actions = quickfixes("file:///m.sql", &[diag, plain]);
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].title, "Change to 'stg_orders'");
        assert_eq!(actions[0].kind, Some("quickfix"));
        let edit = actions[0].edit.as_ref().expect("edit");
        let wire = serde_json::to_value(edit).unwrap();
        assert_eq!(
            wire["documentChanges"][0]["edits"][0]["newText"], "stg_orders",
            "{wire}"
        );
    }
}
