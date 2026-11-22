//! Declare what a `source()` or `doc()` names but no file declares, at
//! the cursor — the counterparts of `define_missing.rs` for the YAML
//! and Markdown side of a project:
//!
//! - `source('s', 't')` with no source `s`: "Declare source 's'", a new
//!   entry with the table under the project's existing `sources:` (or a
//!   new `sources.yml` beside the model when the project has none);
//! - a known source without table `t`: "Add table 't' to source 's'";
//! - `doc('x')` with no docs block: "Create docs block 'x'", appended to
//!   a docs file beside the referencing file (or a new `docs.md` there).
//!
//! Pure: the shell hands in the files' text through `read`.

use std::path::{Path, PathBuf};

use cogitator_lsp::{
    CodeAction, CreateFile, DocumentChange, Position, Range, TextDocumentEdit, TextEdit,
    WorkspaceEdit,
};
use cogitator_parser::TokenKind;

use super::super::{file_uri, DbtContext, Document};
use super::yaml_text::{
    append_to_list, block_end, indent, insert_after_line, is_content, plain_name,
};

pub fn declare_missing_actions(
    ctx: &DbtContext,
    doc: &Document,
    uri: &str,
    range: Range,
    read: &dyn Fn(&Path) -> Option<String>,
) -> Vec<CodeAction> {
    let parsed = &doc.parsed;
    let Some(idx) = parsed.find_token_at_cursor(range.start.line, range.start.character) else {
        return Vec::new();
    };
    let token = &parsed.tokens[idx];
    let literal = parsed.literal(token);
    let here = cogitator_lsp::uri_to_fs_path(uri).map(PathBuf::from);
    let dir = here.as_deref().and_then(Path::parent);
    let action = match token.kind {
        TokenKind::SourceName if !ctx.source_detail_map.contains_key(literal) => {
            // the table is the next source-table token of the same call
            let table = parsed.tokens[idx + 1..]
                .iter()
                .take(6)
                .find(|t| t.kind == TokenKind::SourceTable)
                .map(|t| parsed.literal(t));
            table.and_then(|table| declare_source(ctx, dir?, literal, table, read))
        }
        TokenKind::SourceTable => {
            let source = parsed.tokens[..idx]
                .iter()
                .rev()
                .take(6)
                .find(|t| t.kind == TokenKind::SourceName)
                .map(|t| parsed.literal(t));
            source
                .and_then(|s| ctx.source_detail_map.get(s))
                .filter(|s| !s.tables.contains_key(literal))
                .and_then(|source| {
                    let text = read(&source.uri)?;
                    let name = plain_name(literal)?;
                    let line = source.range.start.line as usize;
                    let edit =
                        append_to_list(&text, line, "tables", &[vec![format!("- name: {name}")]])?;
                    Some(edit_action(
                        format!("Add table '{name}' to source '{}'", source.name),
                        vec![edit_of(&source.uri, edit)],
                    ))
                })
        }
        TokenKind::Doc if !ctx.doc_detail_map.contains_key(literal) => {
            dir.and_then(|dir| create_doc_block(ctx, dir, literal, read))
        }
        _ => None,
    };
    action.into_iter().collect()
}

fn declare_source(
    ctx: &DbtContext,
    dir: &Path,
    source: &str,
    table: &str,
    read: &dyn Fn(&Path) -> Option<String>,
) -> Option<CodeAction> {
    let source = plain_name(source)?;
    let table = plain_name(table)?;
    let title = format!("Declare source '{source}' with table '{table}'");
    // beside the sources the project already declares, by the first name
    let existing = ctx
        .source_detail_map
        .values()
        .min_by(|a, b| a.name.cmp(&b.name))
        .map(|s| s.uri.clone());
    let Some(path) = existing else {
        let path = dir.join("sources.yml");
        if read(&path).is_some() {
            return None; // a file of that name already: hands off
        }
        let content = format!(
            "version: 2\n\nsources:\n  - name: {source}\n    tables:\n      - name: {table}\n"
        );
        let uri = file_uri(&path);
        return Some(edit_action(
            title,
            vec![
                DocumentChange::Create(CreateFile::new(uri.clone())),
                edit_at_start(uri, content),
            ],
        ));
    };
    let text = read(&path)?;
    let lines: Vec<&str> = text.lines().collect();
    let sources = lines
        .iter()
        .position(|l| indent(l) == 0 && l.trim_end() == "sources:")?;
    let end = block_end(&lines, sources, 0);
    let item = (sources + 1..=end)
        .find(|&i| is_content(lines[i]) && lines[i].trim_start().starts_with('-'))
        .map_or(2, |i| indent(lines[i]));
    let pad = " ".repeat(item);
    let body = format!("{pad}- name: {source}\n{pad}  tables:\n{pad}    - name: {table}\n");
    let edit = insert_after_line(&text, end, body);
    Some(edit_action(title, vec![edit_of(&path, edit)]))
}

fn create_doc_block(
    ctx: &DbtContext,
    dir: &Path,
    name: &str,
    read: &dyn Fn(&Path) -> Option<String>,
) -> Option<CodeAction> {
    let name = plain_name(name)?;
    let title = format!("Create docs block '{name}'");
    let block = format!("{{% docs {name} %}}\n\n{{% enddocs %}}\n");
    // a docs file beside the referencing file, the first by path
    let beside = ctx
        .doc_detail_map
        .values()
        .map(|d| &d.uri)
        .filter(|p| p.parent() == Some(dir))
        .min();
    let Some(path) = beside else {
        let path = dir.join("docs.md");
        if read(&path).is_some() {
            return None;
        }
        let uri = file_uri(&path);
        return Some(edit_action(
            title,
            vec![
                DocumentChange::Create(CreateFile::new(uri.clone())),
                edit_at_start(uri, block),
            ],
        ));
    };
    let text = read(path)?;
    let last = text.lines().count().saturating_sub(1);
    let edit = insert_after_line(&text, last, format!("\n{block}"));
    Some(edit_action(title, vec![edit_of(path, edit)]))
}

fn edit_of(path: &Path, edit: TextEdit) -> DocumentChange {
    DocumentChange::Edit(TextDocumentEdit::new(file_uri(path), vec![edit]))
}

fn edit_at_start(uri: String, new_text: String) -> DocumentChange {
    DocumentChange::Edit(TextDocumentEdit::new(
        uri,
        vec![TextEdit {
            range: Range::zero_width(Position {
                line: 0,
                character: 0,
            }),
            new_text,
        }],
    ))
}

fn edit_action(title: String, document_changes: Vec<DocumentChange>) -> CodeAction {
    CodeAction {
        title,
        kind: Some("quickfix"),
        diagnostics: None,
        edit: Some(WorkspaceEdit { document_changes }),
        command: None,
        is_preferred: None,
    }
}
