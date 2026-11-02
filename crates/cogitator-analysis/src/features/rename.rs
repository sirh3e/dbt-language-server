//! textDocument/rename for models: the project graph knows every dependent,
//! so a rename is (1) rewrite each `ref('old')`, (2) update the properties-
//! YAML entry, (3) rename the model file — returned as one WorkspaceEdit
//! (`documentChanges`, text edits before the file rename).
//!
//! Dependent files are read from disk; unsaved editor buffers of *other*
//! files are not visible to the server and keep their text until the client
//! applies the edit.

use cogitator_parser::{DocumentParser, JinjaSqlParser, TokenKind};
use serde_json::{json, Value};

use super::{file_uri, model_name_from_uri, token_range, DbtContext, Document};
use cogitator_lsp::Position;

pub fn rename_model(
    ctx: &DbtContext,
    doc: Option<&Document>,
    uri: &str,
    pos: Position,
    new_name: &str,
) -> Option<Value> {
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

    let mut document_changes: Vec<Value> = Vec::new();

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
        let edits: Vec<Value> = parsed
            .tokens
            .iter()
            .filter(|t| t.kind == TokenKind::Ref && parsed.literal(t) == model)
            .map(|t| json!({ "range": token_range(t), "newText": new_name }))
            .collect();
        if !edits.is_empty() {
            document_changes.push(json!({
                "textDocument": { "uri": file_uri(&path), "version": Value::Null },
                "edits": edits,
            }));
        }
    }

    // 2. the properties-YAML name entry
    if let Some(schema_uri) = &details.schema_uri {
        let start = details.schema_range.start;
        document_changes.push(json!({
            "textDocument": { "uri": file_uri(schema_uri), "version": Value::Null },
            "edits": [{
                "range": {
                    "start": start,
                    "end": { "line": start.line, "character": start.character + model.len() as u32 },
                },
                "newText": new_name,
            }],
        }));
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
    document_changes.push(json!({
        "kind": "rename",
        "oldUri": file_uri(&details.uri),
        "newUri": file_uri(&new_path),
    }));

    Some(json!({ "documentChanges": document_changes }))
}
