//! textDocument/rename for models: the project graph knows every dependent,
//! so a rename is (1) rewrite each `ref('old')`, (2) update the properties-
//! YAML entry, (3) rename the model file — returned as one WorkspaceEdit
//! (`documentChanges`, text edits before the file rename).
//!
//! Dependent files are read from disk; unsaved editor buffers of *other*
//! files are not visible to the server and keep their text until the client
//! applies the edit.

use cogitator_lsp::{
    DocumentChange, Position, Range, RenameFile, TextDocumentEdit, TextEdit, WorkspaceEdit,
};
use cogitator_parser::{DocumentParser, JinjaSqlParser, TokenKind};

use super::{file_uri, model_name_from_uri, token_range, DbtContext, Document};

/// textDocument/prepareRename: only a `ref('…')` literal under the
/// cursor is renameable from anywhere; everything else answers null
/// and the client never opens the rename box.
pub fn prepare_rename(doc: &Document, pos: Position) -> Option<cogitator_lsp::PrepareRenameResult> {
    let idx = doc.parsed.find_token_at_cursor(pos.line, pos.character)?;
    let token = &doc.parsed.tokens[idx];
    (token.kind == TokenKind::Ref).then(|| cogitator_lsp::PrepareRenameResult {
        range: token_range(token),
        placeholder: doc.parsed.literal(token).to_string(),
    })
}

pub fn rename_model(
    ctx: &DbtContext,
    doc: Option<&Document>,
    uri: &str,
    pos: Position,
    new_name: &str,
) -> Option<WorkspaceEdit> {
    if new_name.is_empty() || !new_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return None;
    }

    // target: the ref under the cursor, else the open file's own model
    let model = doc
        .and_then(|d| {
            d.parsed
                .find_token_at_cursor(pos.line, pos.character)
                .and_then(|idx| {
                    let t = &d.parsed.tokens[idx];
                    (t.kind == TokenKind::Ref).then(|| d.parsed.literal(t).to_string())
                })
        })
        .or_else(|| model_name_from_uri(uri))?;
    let details = ctx.model_detail_map.get(&model)?;

    let mut document_changes: Vec<DocumentChange> = Vec::new();

    // 1. rewrite ref('old') in every dependent (and the model's own file)
    let mut files: Vec<std::path::PathBuf> = ctx
        .graph
        .downstream_of(&model)
        .map(|node| node.uri.clone())
        .filter(|p| !p.as_os_str().is_empty())
        .collect();
    files.sort();
    files.dedup();
    for path in files {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(parsed) = JinjaSqlParser.parse(&text, &ctx.dialect) else {
            continue;
        };
        let edits: Vec<TextEdit> = parsed
            .tokens
            .iter()
            .filter(|t| t.kind == TokenKind::Ref && parsed.literal(t) == model)
            .map(|t| TextEdit {
                range: token_range(t),
                new_text: new_name.to_string(),
            })
            .collect();
        if !edits.is_empty() {
            document_changes.push(DocumentChange::Edit(TextDocumentEdit::new(
                file_uri(&path),
                edits,
            )));
        }
    }

    // 2. the properties-YAML name entry
    if let Some(schema_uri) = &details.schema_uri {
        let start = details.schema_range.start;
        document_changes.push(DocumentChange::Edit(TextDocumentEdit::new(
            file_uri(schema_uri),
            vec![TextEdit {
                range: Range {
                    start,
                    end: Position {
                        line: start.line,
                        character: start.character + model.len() as u32,
                    },
                },
                new_text: new_name.to_string(),
            }],
        )));
    }

    // 3. rename the model file itself (last: text edits address old uris)
    let extension = details
        .uri
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("sql");
    let new_path = details
        .uri
        .with_file_name(format!("{new_name}.{extension}"));
    document_changes.push(DocumentChange::Rename(RenameFile::new(
        file_uri(&details.uri),
        file_uri(&new_path),
    )));

    Some(WorkspaceEdit { document_changes })
}
