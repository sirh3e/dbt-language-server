//! "Inherit description of 'col' from <upstream>": a column's
//! description copied from the column it passes through unchanged,
//! found by the schema-YAML sync's walk ([`walk`]): lineage
//! `direct` only (a computed or renamed column inherits nothing), any
//! number of undocumented models in between, a model or a source
//! column at the end. Offered on an undocumented column entry of a
//! properties file and on a select item's output name in the model's
//! SQL, which writes the entry (or the file) it lacks.
//!
//! The description is copied as written: an upstream `{{ doc('x') }}`
//! becomes the same `{{ doc('x') }}` here, never its expanded text, so
//! the doc block stays the one place the words live.

use std::path::Path;

use cogitator_lsp::{
    CodeAction, CreateFile, DocumentChange, Position, Range, TextDocumentEdit, TextEdit,
    WorkspaceEdit,
};
use cogitator_parser::{Keyword, TokenKind};

use super::super::yaml_sync::inherit::{walk, Found, Lineages, Walk};
use super::super::{file_uri, model_name_from_uri, DbtContext, Document};
use super::raw_description::written;
use super::yaml_fixes::insert_under_column;
use super::yaml_text::{append_to_list, plain_name};
use crate::context::ModelDetails;
use crate::lineage::{ModelLineage, SourceRef};

/// A model's column lineage, as the shell computes it (typed where it can).
pub type LineageOf<'a> = &'a dyn Fn(&str) -> Option<ModelLineage>;

/// One action per undocumented column of the entry of `model` in the
/// properties file `text` (at `uri`) with a documented upstream.
pub fn inherit_description_actions(
    ctx: &DbtContext,
    text: &str,
    uri: &str,
    model: &str,
    details: &ModelDetails,
    lineage: LineageOf<'_>,
    read: &dyn Fn(&Path) -> Option<String>,
) -> Vec<CodeAction> {
    let mut lineages = Lineages::new(lineage);
    let mut out = Vec::new();
    for column in &details.columns {
        if !column.description.trim().is_empty() {
            continue;
        }
        let Walk::Found(found) = walk(ctx, &mut lineages, model, &column.name) else {
            continue;
        };
        let Some(edit) =
            insert_under_column(text, column.name_pos, &description_line(ctx, &found, read))
        else {
            continue;
        };
        out.push(edit_action(
            title(&column.name, &found),
            vec![DocumentChange::Edit(TextDocumentEdit::new(
                uri.to_string(),
                vec![edit],
            ))],
        ));
    }
    out.truncate(5); // a menu, not a wall
    out
}

/// On an output name of a select item in a model's SQL: the same
/// description, written into the model's properties YAML — under the
/// column's entry, as a new column entry, or in a new `<model>.yml`.
pub fn inherit_description_sql_action(
    ctx: &DbtContext,
    doc: &Document,
    uri: &str,
    range: Range,
    lineage: LineageOf<'_>,
    read: &dyn Fn(&Path) -> Option<String>,
) -> Option<CodeAction> {
    let model = model_name_from_uri(uri)?;
    let details = ctx.model_detail_map.get(&model)?;
    let parsed = &doc.parsed;
    let at = parsed.find_token_at_cursor(range.start.line, range.start.character)?;
    let token = &parsed.tokens[at];
    if !matches!(token.kind, TokenKind::Word(_) | TokenKind::Ident) {
        return None;
    }
    // the output name ends its select item: a comma or FROM follows
    let next = parsed.tokens.get(at + 1).map(|t| t.kind);
    if !matches!(
        next,
        Some(TokenKind::Symbol(',')) | Some(TokenKind::Word(Keyword::FROM))
    ) {
        return None;
    }
    let word = parsed.literal(token);
    let own = lineage(&model)?;
    let column = own
        .columns
        .iter()
        .find(|c| !c.name.contains('*') && c.name.eq_ignore_ascii_case(word))?
        .name
        .clone();
    let entry = details
        .columns
        .iter()
        .find(|c| c.name.eq_ignore_ascii_case(&column));
    if entry.is_some_and(|c| !c.description.trim().is_empty()) {
        return None; // a written description is never replaced
    }
    let mut lineages = Lineages::new(lineage);
    let Walk::Found(found) = walk(ctx, &mut lineages, &model, &column) else {
        return None;
    };
    let line = description_line(ctx, &found, read);
    let changes = match (&details.schema_uri, entry) {
        (Some(schema), Some(entry)) => {
            let text = read(schema)?;
            let edit = insert_under_column(&text, entry.name_pos, &line)?;
            vec![DocumentChange::Edit(TextDocumentEdit::new(
                file_uri(schema),
                vec![edit],
            ))]
        }
        (Some(schema), None) => {
            let text = read(schema)?;
            let name = plain_name(&column)?;
            let item = vec![format!("- name: {name}"), format!("  {line}")];
            let edit = append_to_list(
                &text,
                details.schema_range.start.line as usize,
                "columns",
                &[item],
            )?;
            vec![DocumentChange::Edit(TextDocumentEdit::new(
                file_uri(schema),
                vec![edit],
            ))]
        }
        (None, _) => {
            let path = details.uri.with_file_name(format!("{model}.yml"));
            if read(&path).is_some() {
                return None; // a file is there but holds no entry: hands off
            }
            let name = plain_name(&column)?;
            let content = format!(
                "version: 2\n\nmodels:\n  - name: {model}\n    columns:\n      - name: {name}\n        {line}\n"
            );
            let new_uri = file_uri(&path);
            vec![
                DocumentChange::Create(CreateFile::new(new_uri.clone())),
                DocumentChange::Edit(TextDocumentEdit::new(
                    new_uri,
                    vec![TextEdit {
                        range: Range::zero_width(Position::default()),
                        new_text: content,
                    }],
                )),
            ]
        }
    };
    Some(edit_action(title(&column, &found), changes))
}

/// `description: "<as written upstream>"`, JSON quoting being YAML's:
/// the written text where it can be read, so a doc() call stays one.
pub(crate) fn description_line(
    ctx: &DbtContext,
    found: &Found,
    read: &dyn Fn(&Path) -> Option<String>,
) -> String {
    let text = written(ctx, &found.from, read).unwrap_or_else(|| found.description.clone());
    format!(
        "description: {}",
        serde_json::to_string(&text).expect("a string serializes")
    )
}

/// What the description was read from, for a title.
pub(crate) fn upstream_label(found: &Found) -> String {
    let from = match &found.from {
        SourceRef::Model { name, column } => format!("{name}.{column}"),
        SourceRef::Source {
            source,
            table,
            column,
        } => format!("source {source}.{table}.{column}"),
        SourceRef::Unknown { name, column } => format!("{name}.{column}"),
    };
    if found.via.is_empty() {
        from
    } else {
        format!("{from} (via {})", found.via.join(", "))
    }
}

fn title(column: &str, found: &Found) -> String {
    format!(
        "Inherit description of '{column}' from {}",
        upstream_label(found)
    )
}

fn edit_action(title: String, document_changes: Vec<DocumentChange>) -> CodeAction {
    CodeAction {
        title,
        kind: Some("quickfix"),
        diagnostics: None,
        edit: Some(WorkspaceEdit { document_changes }),
        command: None,
        is_preferred: None,
        data: None,
    }
}
