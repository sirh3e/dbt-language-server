//! What the cursor names, for the project-wide navigation features: a
//! source (whole or one table), a macro (at a call site, its definition
//! header or a `macros:` YAML entry), a docs block (a `doc()` use or the
//! `{% docs %}` header) or a documented model column (its YAML entry or
//! a word of SQL). Pure over the context and the document.

use cogitator_lsp::Position;
use cogitator_parser::TokenKind;

use super::super::{model_name_from_uri, DbtContext, Document};
use super::files::{is_yaml, path_of, project_of_path, resolve_macro};
use super::occur::{contains, docs_def_at, macro_def_at, name_span, qualifier, unquoted};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Target {
    Source {
        source: String,
        table: Option<String>,
    },
    /// `package` is the project the macro is defined in, resolved.
    Macro {
        package: String,
        name: String,
    },
    Doc(String),
    Column {
        model: String,
        column: String,
    },
}

pub(crate) fn target_at(
    ctx: &DbtContext,
    doc: &Document,
    uri: &str,
    pos: Position,
) -> Option<Target> {
    token_target(ctx, doc, uri, pos)
        .or_else(|| yaml_target(ctx, doc, uri, pos))
        .or_else(|| definition_target(ctx, doc, uri, pos))
        .or_else(|| column_word_target(ctx, doc, uri, pos))
}

/// A refined dbt token under the cursor.
fn token_target(ctx: &DbtContext, doc: &Document, uri: &str, pos: Position) -> Option<Target> {
    let parsed = &doc.parsed;
    let idx = parsed.find_token_at_cursor(pos.line, pos.character)?;
    let token = &parsed.tokens[idx];
    let literal = parsed.literal(token);
    match token.kind {
        TokenKind::SourceName => Some(Target::Source {
            source: literal.to_string(),
            table: None,
        }),
        TokenKind::SourceTable => parsed
            .lookback_find(idx, TokenKind::SourceName, 4)
            .map(|s| Target::Source {
                source: parsed.literal(s).to_string(),
                table: Some(literal.to_string()),
            }),
        TokenKind::Doc => Some(Target::Doc(literal.to_string())),
        TokenKind::Macro => {
            let from =
                path_of(uri).map_or_else(|| ctx.project.name.clone(), |p| project_of_path(ctx, &p));
            let details = resolve_macro(ctx, &from, qualifier(parsed, idx), literal)?;
            Some(Target::Macro {
                package: details.project_name.clone(),
                name: literal.to_string(),
            })
        }
        _ => None,
    }
}

/// A name of the properties YAML: a model column, a source or source
/// table, or a `macros:` entry.
fn yaml_target(ctx: &DbtContext, doc: &Document, uri: &str, pos: Position) -> Option<Target> {
    let path = path_of(uri)?;
    if !is_yaml(&path) {
        return None;
    }
    let text = &doc.text;
    let hit =
        |at: Position, name: &str| name_span(text, at, name).is_some_and(|r| contains(&r, pos));
    for (model, details) in &ctx.model_detail_map {
        if details.schema_uri.as_deref() != Some(path.as_path()) {
            continue;
        }
        if let Some(column) = details.columns.iter().find(|c| hit(c.name_pos, &c.name)) {
            return Some(Target::Column {
                model: model.clone(),
                column: column.name.clone(),
            });
        }
    }
    for source in ctx.source_detail_map.values().filter(|s| s.uri == path) {
        if hit(source.range.start, &source.name) {
            return Some(Target::Source {
                source: source.name.clone(),
                table: None,
            });
        }
        if let Some(table) = source.tables.values().find(|t| hit(t.range.start, &t.name)) {
            return Some(Target::Source {
                source: source.name.clone(),
                table: Some(table.name.clone()),
            });
        }
    }
    let name = super::yaml_calls::macro_entry_at(text, pos)?;
    let details = resolve_macro(ctx, &ctx.project.name, None, &name)?;
    Some(Target::Macro {
        package: details.project_name.clone(),
        name,
    })
}

/// A `{% macro name(…) %}` header or a `{% docs name %}` header.
fn definition_target(ctx: &DbtContext, doc: &Document, uri: &str, pos: Position) -> Option<Target> {
    if let Some(raw) = macro_def_at(&doc.text, pos) {
        let project =
            path_of(uri).map_or_else(|| ctx.project.name.clone(), |p| project_of_path(ctx, &p));
        return Some(Target::Macro {
            package: project,
            name: raw.name,
        });
    }
    docs_def_at(&doc.text, pos).map(Target::Doc)
}

/// A SQL word naming a documented column: the file's own model's first,
/// else a column of a model the file selects from.
fn column_word_target(
    ctx: &DbtContext,
    doc: &Document,
    uri: &str,
    pos: Position,
) -> Option<Target> {
    let model = model_name_from_uri(uri)?;
    let parsed = &doc.parsed;
    let idx = parsed.find_token_at_cursor(pos.line, pos.character)?;
    let token = &parsed.tokens[idx];
    if !matches!(token.kind, TokenKind::Word(_) | TokenKind::Ident) {
        return None;
    }
    let word = unquoted(parsed.literal(token));
    let documented = |m: &str| {
        ctx.model_detail_map
            .get(m)
            .and_then(|d| d.columns.iter().find(|c| c.name.eq_ignore_ascii_case(word)))
            .map(|c| c.name.clone())
    };
    if let Some(column) = documented(&model) {
        return Some(Target::Column { model, column });
    }
    let hit = super::super::completion::scope::resolve_column(ctx, doc, word)?;
    let column = documented(&hit.relation)?;
    Some(Target::Column {
        model: hit.relation,
        column,
    })
}
