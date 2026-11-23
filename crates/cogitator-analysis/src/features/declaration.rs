//! textDocument/declaration, and the YAML-side half of definition: a
//! model is *declared* by its properties-YAML entry and *defined* by its
//! SQL; a documented column is declared by its YAML column entry and
//! defined where the model's SQL produces it. Sources, macros, vars and
//! docs have one place, so declaration answers what definition does.
//! Pure: the one file a YAML column's definition reads comes through a
//! `read` closure.

use std::path::Path;

use cogitator_lsp::{Location, Position, Range};
use cogitator_parser::{DocumentParser, JinjaSqlParser, Keyword, ParsedDocument, TokenKind};

use super::references::files::Reader;
use super::references::occur::{name_span, unquoted};
use super::references::target::{target_at, yaml_column_at, Target};
use super::{definition, file_uri, model_name_from_uri, token_range, DbtContext, Document};

/// Declaration (falls back to definition wherever there is no YAML).
pub fn declaration(ctx: &DbtContext, doc: &Document, uri: &str, pos: Position) -> Location {
    declared(ctx, doc, uri, pos).unwrap_or_else(|| definition(ctx, doc, uri, pos))
}

fn declared(ctx: &DbtContext, doc: &Document, uri: &str, pos: Position) -> Option<Location> {
    let open = Some((uri, &*doc.text));
    let parsed = &doc.parsed;
    // a ref under the cursor: the model's properties entry
    if let Some(idx) = parsed.find_token_at_cursor(pos.line, pos.character) {
        let token = &parsed.tokens[idx];
        if token.kind == TokenKind::Ref {
            return model_entry(ctx, parsed.literal(token), open);
        }
    }
    match target_at(ctx, doc, uri, pos) {
        Some(Target::Column { model, column }) => return column_entry(ctx, &model, &column, open),
        Some(_) => return None,
        None => {}
    }
    // anywhere else in a model file: its own entry
    model_name_from_uri(uri).and_then(|model| model_entry(ctx, &model, open))
}

/// The `name:` of a model's properties-YAML entry, when it has one.
fn model_entry(ctx: &DbtContext, model: &str, open: Option<(&str, &str)>) -> Option<Location> {
    let details = ctx.model_detail_map.get(model)?;
    let schema = details
        .schema_uri
        .as_deref()
        .filter(|p| !p.as_os_str().is_empty())?;
    Some(yaml_name(schema, details.schema_range.start, model, open))
}

/// The `name:` of a documented column's YAML entry.
fn column_entry(
    ctx: &DbtContext,
    model: &str,
    column: &str,
    open: Option<(&str, &str)>,
) -> Option<Location> {
    let details = ctx.model_detail_map.get(model)?;
    let schema = details.schema_uri.as_deref()?;
    let entry = details.columns.iter().find(|c| c.name == column)?;
    Some(yaml_name(schema, entry.name_pos, &entry.name, open))
}

/// A YAML name's location: exact from the open buffer of that file,
/// else its recorded start and length (no disk read on this path).
fn yaml_name(path: &Path, at: Position, name: &str, open: Option<(&str, &str)>) -> Location {
    let text = open
        .filter(|(u, _)| super::references::files::path_of(u).as_deref() == Some(path))
        .map(|(_, t)| t);
    let range = text.and_then(|t| name_span(t, at, name)).unwrap_or(Range {
        start: at,
        end: Position {
            line: at.line,
            character: at.character + name.len() as u32,
        },
    });
    Location {
        uri: file_uri(path),
        range,
    }
}

/// The documented column whose YAML entry the cursor is on, when the
/// document is a properties file: `(model, column)`.
pub fn yaml_column_target(
    ctx: &DbtContext,
    doc: &Document,
    uri: &str,
    pos: Position,
) -> Option<(String, String)> {
    yaml_column_at(ctx, doc, uri, pos)
}

/// Definition of a YAML column entry: where the model's SQL produces
/// the column (see [`production_range`]); the model file's start when
/// the SQL never names it (a `select *` pass-through).
pub fn yaml_column_definition(
    ctx: &DbtContext,
    model: &str,
    column: &str,
    open: Option<(&str, &str)>,
    read: &dyn Fn(&Path) -> Option<String>,
) -> Option<Location> {
    let details = ctx.model_detail_map.get(model)?;
    if details.uri.as_os_str().is_empty() {
        return None;
    }
    let reader = Reader::with(open, read);
    let range = reader
        .read(&details.uri)
        .and_then(|text| JinjaSqlParser.parse(&text, &ctx.dialect).ok())
        .and_then(|parsed| production_range(&parsed, column))
        .unwrap_or_default();
    Some(Location {
        uri: file_uri(&details.uri),
        range,
    })
}

/// Where a SQL text produces `column`: the last select-list item that
/// names it — an `as column` alias, or `[qualifier.]column` ending an
/// item (followed by `,` or `from`). The last one, since a model's
/// final select comes after its CTEs; else its first mention at all.
pub fn production_range(parsed: &ParsedDocument, column: &str) -> Option<Range> {
    let tokens = &parsed.tokens;
    let is_column = |i: usize| {
        matches!(tokens[i].kind, TokenKind::Word(_) | TokenKind::Ident)
            && unquoted(parsed.literal(&tokens[i])).eq_ignore_ascii_case(column)
    };
    let produces = |i: usize| {
        let aliased = i > 0 && tokens[i - 1].kind == TokenKind::Word(Keyword::AS);
        let ends_item = matches!(
            tokens.get(i + 1).map(|t| t.kind),
            Some(TokenKind::Symbol(',') | TokenKind::Word(Keyword::FROM))
        );
        aliased || ends_item
    };
    let names: Vec<usize> = (0..tokens.len()).filter(|&i| is_column(i)).collect();
    names
        .iter()
        .rev()
        .find(|&&i| produces(i))
        .or(names.first())
        .map(|&i| token_range(&tokens[i]))
}

#[cfg(test)]
mod tests;
