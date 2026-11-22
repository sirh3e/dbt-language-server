//! textDocument/rename for models: the project graph knows every dependent,
//! so a rename is (1) rewrite each `ref('old')` — in SQL, in YAML Jinja and
//! in plain YAML scalars (relationship tests' `to:`, exposures'
//! `depends_on:`), (2) update the properties-YAML entry, (3) rename the
//! model file — returned as one WorkspaceEdit (`documentChanges`, one text
//! edit per file before the file rename).
//!
//! Dependent files are read from disk; unsaved editor buffers of *other*
//! files are not visible to the server and keep their text until the client
//! applies the edit.

use std::collections::BTreeMap;
use std::path::PathBuf;

use cogitator_lsp::{
    DocumentChange, Position, Range, RenameFile, TextDocumentEdit, TextEdit, WorkspaceEdit,
};
use cogitator_parser::{DocumentParser, JinjaSqlParser, TokenKind};

use super::references::files::{is_yaml, node_files, yaml_files};
use super::references::occur::name_span;
use super::references::yaml_calls::yaml_calls;
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

    let mut edits: BTreeMap<PathBuf, Vec<Range>> = BTreeMap::new();

    // 1. ref('old') in every dependent, and plain-YAML refs project-wide
    let mut files = node_files(ctx.graph.downstream_of(&model));
    files.extend(yaml_files(ctx));
    files.sort();
    files.dedup();
    for path in files {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let mut ranges: Vec<Range> = match JinjaSqlParser.parse(&text, &ctx.dialect) {
            Ok(parsed) => parsed
                .tokens
                .iter()
                .filter(|t| t.kind == TokenKind::Ref && parsed.literal(t) == model)
                .map(token_range)
                .collect(),
            Err(_) => Vec::new(),
        };
        if is_yaml(&path) {
            ranges.extend(
                yaml_calls(&text, &ctx.dialect)
                    .into_iter()
                    .filter(|c| c.kind == TokenKind::Ref && c.literal == model)
                    .map(|c| c.range),
            );
        }
        if !ranges.is_empty() {
            edits.entry(path).or_default().extend(ranges);
        }
    }

    // 2. the properties-YAML name entry (quotes kept)
    if let Some(schema_uri) = details
        .schema_uri
        .as_ref()
        .filter(|p| !p.as_os_str().is_empty())
    {
        let start = details.schema_range.start;
        let range = std::fs::read_to_string(schema_uri)
            .ok()
            .and_then(|text| name_span(&text, start, &model))
            .unwrap_or(Range {
                start,
                end: Position {
                    line: start.line,
                    character: start.character + model.len() as u32,
                },
            });
        edits.entry(schema_uri.clone()).or_default().push(range);
    }

    let mut document_changes: Vec<DocumentChange> = edits
        .into_iter()
        .map(|(path, mut ranges)| {
            ranges.sort_by_key(|r| (r.start.line, r.start.character));
            ranges.dedup();
            let edits = ranges
                .into_iter()
                .map(|range| TextEdit {
                    range,
                    new_text: new_name.to_string(),
                })
                .collect();
            DocumentChange::Edit(TextDocumentEdit::new(file_uri(&path), edits))
        })
        .collect();

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
