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
    // a `vars:` key in dbt_project.yml: where its value is decided
    if let Some(loc) = super::key_definition(ctx, uri, pos) {
        return loc;
    }
    let Some(idx) = doc.parsed.find_token_at_cursor(pos.line, pos.character) else {
        return fallback;
    };
    let parsed = &doc.parsed;
    // a macro named inside a string: `post_hook="{{ m() }}"`
    if let Some(m) = macro_in_string(ctx, parsed, idx, pos) {
        return Location {
            uri: file_uri(&m.uri),
            range: m.range,
        };
    }
    let idx = named_token(parsed, idx);
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
        // the KEY of the entry that decided the value; an undefined var
        // with a default goes to its default argument
        TokenKind::Var => match ctx.var_for(&doc.uri, literal) {
            Some(v) if !v.uri.as_os_str().is_empty() => Location {
                uri: file_uri(&v.uri),
                range: v.range,
            },
            Some(_) => fallback,
            None => parsed
                .var_call(idx)
                .default
                .map(|(s, e)| Location {
                    uri: uri.to_string(),
                    range: Range {
                        start: super::byte_position(&parsed.text, s),
                        end: super::byte_position(&parsed.text, e),
                    },
                })
                .unwrap_or(fallback),
        },
        TokenKind::EnvVar => {
            super::env_vars::env_definition(ctx, doc, uri, idx).unwrap_or(fallback)
        }
        TokenKind::Doc => ctx
            .doc_detail_map
            .get(literal)
            .map(|d| Location {
                uri: file_uri(&d.uri),
                range: d.range,
            })
            .unwrap_or(fallback),
        TokenKind::Function => ctx
            .function_detail_map
            .get(literal)
            .and_then(|f| f.definition())
            .map(|(path, range)| Location {
                uri: file_uri(&path),
                range,
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

/// The kinds of token a definition or a hover is about.
fn is_named(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Ref
            | TokenKind::SourceName
            | TokenKind::SourceTable
            | TokenKind::Var
            | TokenKind::EnvVar
            | TokenKind::Doc
            | TokenKind::Function
            | TokenKind::Macro
    )
}

/// The functions whose first string arguments name something.
const NAMING_CALLS: &[&str] = &["ref", "source", "var", "doc", "env_var", "function"];

/// The token at `idx`, widened to the call it belongs to: on the word
/// `ref` (`source`, `var`, `doc`, `env_var`), on a package or a project
/// name (`ref('core', 'orders')`, `dbt_utils.star`) or the call's open
/// parenthesis, the name the call is about — the next named token inside
/// the same `{{ … }}`, a few tokens on. Anything else is itself.
pub fn named_token(parsed: &cogitator_parser::ParsedDocument, idx: usize) -> usize {
    let token = &parsed.tokens[idx];
    if is_named(token.kind) {
        return idx;
    }
    let widen = match token.kind {
        TokenKind::Package => true,
        TokenKind::Ident | TokenKind::Word(_) | TokenKind::Config => {
            NAMING_CALLS.contains(&parsed.literal(token))
        }
        TokenKind::Symbol('(') | TokenKind::Symbol('.') => idx
            .checked_sub(1)
            .map(|before| {
                let t = &parsed.tokens[before];
                t.kind == TokenKind::Package
                    || NAMING_CALLS.contains(&parsed.literal(t))
                    || t.kind == TokenKind::Macro
            })
            .unwrap_or(false),
        _ => false,
    };
    if !widen {
        return idx;
    }
    parsed.tokens[idx + 1..]
        .iter()
        .take(6)
        .take_while(|t| !matches!(t.kind, TokenKind::JinjaClose(_)))
        .position(|t| is_named(t.kind))
        .map(|offset| idx + 1 + offset)
        .unwrap_or(idx)
}

/// A macro called inside a string literal at the cursor — the SQL of a
/// hook, `post_hook="{{ market_export() }}"` — found by its name in the
/// project's (or the named package's) macros.
pub fn macro_in_string<'a>(
    ctx: &'a DbtContext,
    parsed: &cogitator_parser::ParsedDocument,
    idx: usize,
    pos: Position,
) -> Option<&'a crate::context::MacroDetails> {
    let token = &parsed.tokens[idx];
    if token.kind != TokenKind::StringLit || token.line != pos.line {
        return None;
    }
    let text = parsed.literal(token);
    let at = (pos.character.checked_sub(token.column)?) as usize;
    if !text.contains("{{") && !text.contains("{%") {
        return None;
    }
    let bytes = text.as_bytes();
    let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'.';
    if at >= bytes.len() || !is_word(bytes[at]) {
        return None;
    }
    let mut start = at;
    while start > 0 && is_word(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = at;
    while end < bytes.len() && is_word(bytes[end]) {
        end += 1;
    }
    // a call: the name is followed by `(`
    if !text[end..].trim_start().starts_with('(') {
        return None;
    }
    let name = &text[start..end];
    let (package, name) = match name.rsplit_once('.') {
        Some((package, name)) => (package.to_string(), name),
        None => (ctx.project.name.clone(), name),
    };
    ctx.macro_detail_map
        .get(&package)
        .and_then(|m| m.get(name))
        .or_else(|| ctx.macro_detail_map.values().find_map(|m| m.get(name)))
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
