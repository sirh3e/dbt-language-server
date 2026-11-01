//! Feature algorithms (specs/rust-port/03 §9): pure functions over the
//! context snapshot and a parsed document; the dispatcher serializes.

use std::path::Path;
use std::sync::LazyLock;

use dbt_ls_lsp::{completion_kind, CompletionItem, Location, Position, Range};
use dbt_ls_parser::TokenKind;
use regex::Regex;

use crate::context::DbtContext;
use crate::Document;

fn file_uri(path: &Path) -> String {
    format!("file://{}", path.display())
}

/// Hover (03 §9.1). Default: empty contents.
pub fn hover(ctx: &DbtContext, doc: &Document, pos: Position) -> String {
    let Some(idx) = doc.parsed.find_token_at_cursor(pos.line, pos.character) else {
        return String::new();
    };
    let parsed = &doc.parsed;
    let token = &parsed.tokens[idx];
    let literal = parsed.literal(token);

    match token.kind {
        TokenKind::Ref => ctx
            .model_detail_map
            .get(literal)
            .map(|m| m.description.clone())
            .unwrap_or_default(),
        TokenKind::SourceName => ctx
            .source_detail_map
            .get(literal)
            .map(|s| s.description.clone())
            .unwrap_or_default(),
        TokenKind::SourceTable => {
            let Some(source_token) = parsed.lookback_find(idx, TokenKind::SourceName, 4) else {
                return String::new();
            };
            let source_name = parsed.literal(source_token);
            let Some(source) = ctx.source_detail_map.get(source_name) else {
                return String::new();
            };
            let table = source.tables.get(literal);
            format!(
                "Source: {}\n{}\n\nTable: {}\n{}",
                source.name,
                source.description,
                table.map(|t| t.name.as_str()).unwrap_or_default(),
                table.map(|t| t.description.as_str()).unwrap_or_default(),
            )
        }
        TokenKind::Var => {
            let value = ctx
                .variable_detail_map
                .get(literal)
                .map(|v| display_value(&v.value))
                .unwrap_or_default();
            format!("{literal}: {value}")
        }
        TokenKind::Macro => {
            let package = parsed
                .lookback_find(idx, TokenKind::Package, 2)
                .map(|t| parsed.literal(t).to_string())
                .unwrap_or_else(|| ctx.project.name.clone());
            ctx.macro_detail_map
                .get(&package)
                .and_then(|m| m.get(literal))
                .map(|m| m.description.clone())
                .unwrap_or_default()
        }
        _ => ctx
            .dialect
            .function_docs()
            .get(literal)
            .cloned()
            .unwrap_or_default(),
    }
}

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
            // CTE jump within the file (03 §9.2 default arm).
            parsed
                .def_tokens
                .get(literal)
                .map(|def| Location {
                    uri: uri.to_string(),
                    range: Range::zero_width(Position {
                        line: def.line,
                        character: def.column,
                    }),
                })
                .unwrap_or(fallback)
        }
    }
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

pub fn model_name_from_uri(uri: &str) -> Option<String> {
    let path = uri.strip_prefix("file://").unwrap_or(uri);
    let base = Path::new(path).file_name()?.to_string_lossy();
    base.strip_suffix(".sql").map(str::to_string)
}

// ---------------------------------------------------------------------------
// Completion (03 §9.4)
// ---------------------------------------------------------------------------

// Q1 fixed: `[a-zA-Z0-9_]` instead of Go's `[a-zA-z]` typo.
static REF_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\bref\(('|")[a-zA-Z0-9_]*$"#).unwrap());
static SOURCE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\bsource\(('|")[a-zA-Z0-9_]*$"#).unwrap());
static VAR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\bvar\(('|")[a-zA-Z0-9_]*$"#).unwrap());
static JINJA_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{\{\s*").unwrap());

pub fn completion(ctx: &DbtContext, doc: &Document, pos: Position) -> Vec<CompletionItem> {
    let lines: Vec<&str> = doc.text.split('\n').collect();
    let Some(&line) = lines.get(pos.line as usize) else {
        return Vec::new(); // Go panicked here (Q8-adjacent); degrade instead
    };
    let cursor = (pos.character as usize).min(line.len());
    let cursor = (0..=cursor)
        .rev()
        .find(|&i| line.is_char_boundary(i))
        .unwrap_or(0);
    let before = &line[..cursor];
    let after = &line[cursor..];

    if REF_RE.is_match(before) {
        ref_items(ctx, &suffix_for(line, after, "ref"))
    } else if SOURCE_RE.is_match(before) {
        source_items(ctx, &suffix_for(line, after, "source"), &quote_type(before))
    } else if VAR_RE.is_match(before) {
        var_items(ctx, &suffix_for(line, after, "var"))
    } else if JINJA_RE.is_match(before) {
        macro_items(ctx)
    } else {
        ctx.dialect.function_completion_items()
    }
}

fn ref_items(ctx: &DbtContext, suffix: &str) -> Vec<CompletionItem> {
    ctx.model_detail_map
        .iter()
        .map(|(name, m)| CompletionItem {
            label: name.clone(),
            detail: format!("Project: {}", m.project_name),
            documentation: m.description.clone(),
            kind: completion_kind::REFERENCE,
            insert_text: format!("{name}{suffix}"),
            sort_text: name.clone(),
        })
        .collect()
}

fn source_items(ctx: &DbtContext, suffix: &str, quote: &str) -> Vec<CompletionItem> {
    ctx.source_detail_map
        .iter()
        .flat_map(|(key, source)| {
            source.tables.values().map(move |table| CompletionItem {
                label: format!("{} - {}", source.name, table.name),
                detail: format!("Source: {}", source.name),
                documentation: format!(
                    "{}\n\nTable: {}\n{}",
                    source.description, table.name, table.description
                ),
                kind: completion_kind::REFERENCE,
                insert_text: format!("{}{quote}, {quote}{}{suffix}", source.name, table.name),
                sort_text: key.clone(),
            })
        })
        .collect()
}

fn var_items(ctx: &DbtContext, suffix: &str) -> Vec<CompletionItem> {
    ctx.variable_detail_map
        .iter()
        .map(|(name, v)| CompletionItem {
            label: name.clone(),
            detail: name.clone(),
            documentation: display_value(&v.value),
            kind: completion_kind::VARIABLE,
            insert_text: format!("{name}{suffix}"),
            sort_text: name.clone(),
        })
        .collect()
}

fn macro_items(ctx: &DbtContext) -> Vec<CompletionItem> {
    ctx.macro_detail_map
        .values()
        .flat_map(|macros| macros.values())
        .map(|m| {
            let insert_text = if ctx.project.name == m.project_name {
                m.name.clone()
            } else {
                format!("{}.{}", m.project_name, m.name)
            };
            CompletionItem {
                label: m.name.clone(),
                detail: format!("Project: {}", m.project_name),
                documentation: m.description.clone(),
                kind: completion_kind::SNIPPET,
                insert_text,
                sort_text: m.name.clone(),
            }
        })
        .collect()
}

/// Closing-suffix mirroring (03 §9.4.1): `{{ ref('` → `') }}`.
fn suffix_for(line: &str, after: &str, kind: &str) -> String {
    if !after.is_empty() {
        return String::new();
    }
    let re = Regex::new(&format!(r#"\{{\{{\s*{kind}\(('|")"#)).expect("static pattern");
    let Some(prefix) = re.find(line).map(|m| m.as_str()) else {
        return String::new();
    };
    let without_kind = prefix.replacen(kind, "", 1);
    without_kind
        .chars()
        .rev()
        .map(|c| match c {
            '(' => ')',
            '{' => '}',
            other => other,
        })
        .collect()
}

/// Quote style for the source item's middle quotes (03 §9.4.2; Q3 fixed:
/// genuinely the last quote before the cursor).
fn quote_type(before: &str) -> String {
    before
        .chars()
        .rev()
        .find(|c| *c == '\'' || *c == '"')
        .map(String::from)
        .unwrap_or_default()
}

pub(crate) fn display_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}
