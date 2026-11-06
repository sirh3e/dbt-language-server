//! textDocument/definition and dbt.goToSchema (03 §9.2–9.3).

use cogitator_lsp::{Location, Position, Range};
use cogitator_parser::TokenKind;

use super::{file_uri, model_name_from_uri, DbtContext, Document};

/// Definition (03 §9.2). Default: zero-width range at the request cursor.
pub fn definition(ctx: &DbtContext, doc: &Document, uri: &str, pos: Position) -> Location {
    let fallback = Location {
        uri: uri.to_string(),
        range: Range::zero_width(pos),
    };
    let Some(idx) = doc.parsed.find_token_at_cursor(pos.line, pos.character) else {
        return fallback;
    };
    let parsed = &doc.parsed;
    let token = &parsed.tokens[idx];
    let literal = parsed.literal(token);

    match token.kind {
        TokenKind::Ref => ctx
            .model_detail_map
            .get(literal)
            .map(|m| Location {
                uri: file_uri(&m.uri),
                range: Range::default(),
            })
            .unwrap_or(fallback),
        TokenKind::SourceName => ctx
            .source_detail_map
            .get(literal)
            .filter(|s| !s.name.is_empty())
            .map(|s| Location {
                uri: file_uri(&s.uri),
                range: s.range,
            })
            .unwrap_or(fallback),
        TokenKind::SourceTable => parsed
            .lookback_find(idx, TokenKind::SourceName, 4)
            .and_then(|src| ctx.source_detail_map.get(parsed.literal(src)))
            .and_then(|s| s.tables.get(literal))
            .filter(|t| !t.name.is_empty())
            .map(|t| Location {
                uri: file_uri(&t.uri),
                range: t.range,
            })
            .unwrap_or(fallback),
        TokenKind::Var => ctx
            .variable_detail_map
            .get(literal)
            .map(|v| Location {
                uri: file_uri(&v.uri),
                range: v.range,
            })
            .unwrap_or(fallback),
        TokenKind::Macro => {
            let package = parsed
                .lookback_find(idx, TokenKind::Package, 2)
                .map(|t| parsed.literal(t).to_string())
                .unwrap_or_else(|| ctx.project.name.clone());
            ctx.macro_detail_map
                .get(&package)
                .and_then(|m| m.get(literal))
                .map(|m| Location {
                    uri: file_uri(&m.uri),
                    range: m.range,
                })
                .unwrap_or(fallback)
        }
        _ => {
            // CTE jump within the file (03 §9.2 default arm) …
            if let Some(def) = parsed.def_tokens.get(literal) {
                return Location {
                    uri: uri.to_string(),
                    range: Range::zero_width(Position {
                        line: def.line,
                        character: def.column,
                    }),
                };
            }
            // … else a documented column jumps to its YAML entry.
            super::completion::scope::resolve_column(ctx, doc, literal)
                .and_then(|hit| hit.yaml)
                .map(|(path, pos)| Location {
                    uri: file_uri(&path),
                    range: Range::zero_width(pos),
                })
                .unwrap_or(fallback)
        }
    }
}

/// What definition-on-a-SQL-function materializes (03 §9): the
/// bundled documentation, bound for a cached Markdown file the shell
/// writes under target/.cogitator/docs/.
#[derive(Debug)]
pub struct FunctionDoc {
    pub dialect: String,
    /// Lowercased — the doc map's key and the cache file's stem.
    pub name: String,
    pub markdown: &'static str,
}

/// Is the cursor on a documented SQL function? Same precedence as the
/// definition `_` arm: a CTE or resolvable column named `sum` still
/// wins over the built-in.
pub fn function_doc_target(ctx: &DbtContext, doc: &Document, pos: Position) -> Option<FunctionDoc> {
    let parsed = &doc.parsed;
    let idx = parsed.find_token_at_cursor(pos.line, pos.character)?;
    let token = &parsed.tokens[idx];
    if !matches!(token.kind, TokenKind::Word(_) | TokenKind::Ident) {
        return None;
    }
    let literal = parsed.literal(token);
    if parsed.def_tokens.contains_key(literal) {
        return None;
    }
    if super::completion::scope::resolve_column(ctx, doc, literal)
        .and_then(|hit| hit.yaml)
        .is_some()
    {
        return None;
    }
    let markdown = ctx.dialect.function_doc(literal)?;
    Some(FunctionDoc {
        dialect: ctx.dialect.as_str().to_string(),
        name: literal.to_lowercase(),
        markdown,
    })
}

/// `dbt.goToSchema` (03 §9.3). `None` serializes as a null result.
pub fn go_to_schema(
    ctx: &DbtContext,
    doc: Option<&Document>,
    uri: &str,
    pos: Position,
) -> Option<Location> {
    if let Some(doc) = doc {
        if let Some(idx) = doc.parsed.find_token_at_cursor(pos.line, pos.character) {
            let token = &doc.parsed.tokens[idx];
            if token.kind == TokenKind::Ref {
                if let Some(location) = schema_location(ctx, doc.parsed.literal(token)) {
                    return Some(location);
                }
            }
        }
    }
    let model_name = model_name_from_uri(uri)?;
    schema_location(ctx, &model_name)
}

fn schema_location(ctx: &DbtContext, model: &str) -> Option<Location> {
    let details = ctx.model_detail_map.get(model)?;
    let schema_uri = details
        .schema_uri
        .as_ref()
        .filter(|p| !p.as_os_str().is_empty())?;
    Some(Location {
        uri: file_uri(schema_uri),
        range: details.schema_range,
    })
}
