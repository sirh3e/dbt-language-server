//! The SQL layer: the template with its Jinja masked byte for byte
//! (`cogitator_duckdb::mask_jinja`, the DuckDB check's masking and its
//! confidence rule), parsed by sqlparser in the project's dialect.
//!
//! A model with control flow (`{% if is_incremental() %}`, `{% for %}`,
//! a block `set`), or an error inside or right after a masked `{{ … }}`,
//! is checked as dbt renders it instead (`render.rs`): both renderings,
//! `is_incremental()` false and true, when the shell gave a renderer, and
//! not at all when it did not or the model cannot be rendered.
//!
//! Declines, each a source of false errors otherwise:
//! - SQL that any other sqlparser dialect accepts: sqlparser covers each
//!   warehouse's grammar only in part, so one dialect's "no" is no proof;
//! - parser limits (recursion) and files past [`MAX_BYTES`].

use std::sync::LazyLock;

use cogitator_dialects::Dialect;
use cogitator_parser::backend::sql_dialect;
use cogitator_parser::{DocumentParser, JinjaKind, JinjaSqlParser, ParsedDocument, TokenKind};
use regex::Regex;
use sqlparser::dialect::{self as sd, Dialect as SqlDialect};
use sqlparser::parser::Parser;

use super::render::SyntaxRenderer;
use super::Finding;

/// Larger templates are not checked as you type.
pub(crate) const MAX_BYTES: usize = 512 * 1024;

static LOCATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\s*at Line: (\d+), Column: (\d+)").expect("location regex compiles")
});

const SOLUTION: &str = "correct the SQL at this position; the warehouse rejects the compiled \
     model the same way. If the SQL is valid for your warehouse, turn the check off with \
     `lint: { syntax: false }` in cogitator.yml and report the case.";

/// The first certain SQL syntax error of `text`. A model whose Jinja
/// is `{{ … }}` alone is checked with it masked, at the exact place; one
/// with control flow (`{% if is_incremental() %}`, `{% for %}`, a block
/// `set`), or an error next to a masked expression, is checked as dbt
/// renders it, when the shell gave a `renderer`.
pub(crate) fn check(
    text: &str,
    dialect: &Dialect,
    model: &str,
    renderer: Option<&SyntaxRenderer>,
) -> Option<Finding> {
    if text.len() > MAX_BYTES {
        return None;
    }
    let parsed = JinjaSqlParser.parse(text, dialect).ok()?;
    let masked = (!has_block_set(&parsed))
        .then(|| cogitator_duckdb::mask_jinja(&parsed))
        .flatten();
    let Some(masked) = masked else {
        trace("control flow: rendering");
        return renderer.and_then(|r| check_rendered(text, dialect, model, r));
    };
    if masked.trim().is_empty() {
        return None;
    }
    let primary = sql_dialect(dialect);
    let Some((offset, detail)) = first_error(primary.as_ref(), &masked) else {
        trace("parsed");
        return None;
    };
    let jinja = jinja_spans(&parsed);
    if near_jinja(&masked, offset, &jinja) {
        trace(&format!("near jinja: rendering ({detail})"));
        return renderer.and_then(|r| check_rendered(text, dialect, model, r));
    }
    if !certain(&masked, offset, &detail) {
        return None;
    }
    trace(&format!("error: {detail}"));
    Some(Finding {
        start: offset,
        end: token_end(&masked, offset),
        headline: format!("SQL syntax error: {detail}"),
        reason: format!(
            "with its Jinja masked, the model is not valid {} SQL, nor valid in any other \
             dialect cogitator knows.",
            dialect_name(dialect)
        ),
        solution: SOLUTION,
    })
}

/// The model rendered both ways dbt renders it — `is_incremental()`
/// false (a full refresh, the first run) and true (every later run) —
/// each rendering checked; an error is one the rendering really has.
fn check_rendered(
    text: &str,
    dialect: &Dialect,
    model: &str,
    renderer: &SyntaxRenderer,
) -> Option<Finding> {
    let primary = sql_dialect(dialect);
    let mut seen: Vec<String> = Vec::new();
    for incremental in [false, true] {
        let Some(sql) = renderer.render(text, model, incremental) else {
            continue;
        };
        if sql.trim().is_empty() || seen.contains(&sql) {
            continue;
        }
        let Some((offset, detail)) = first_error(primary.as_ref(), &sql) else {
            trace(&format!(
                "rendered and parsed (is_incremental {incremental})"
            ));
            seen.push(sql);
            continue;
        };
        if !certain(&sql, offset, &detail) {
            seen.push(sql);
            continue;
        }
        trace(&format!("rendered error ({incremental}): {detail}"));
        let (start, end) = source_span(text, &sql, offset);
        let (line, snippet) = rendered_line(&sql, offset);
        let variant = if !uses_is_incremental(text) {
            "as dbt renders it".to_string()
        } else if incremental {
            "as dbt renders it on an incremental run (`is_incremental()` true)".to_string()
        } else {
            "as dbt renders it on a full refresh (`is_incremental()` false)".to_string()
        };
        return Some(Finding {
            start,
            end,
            headline: format!("SQL syntax error: {detail}"),
            reason: format!(
                "{variant}, the model is not valid {} SQL, nor valid in any other dialect \
                 cogitator knows; line {line} of the rendered SQL reads `{snippet}`.",
                dialect_name(dialect)
            ),
            solution: SOLUTION,
        });
    }
    None
}

/// sqlparser's first error in `sql`: its byte offset and its words;
/// `None` when it parses, or the parser gave up (recursion, no place).
fn first_error(dialect: &dyn SqlDialect, sql: &str) -> Option<(usize, String)> {
    let err = Parser::parse_sql(dialect, sql).err()?.to_string();
    if err.contains("recursion limit") {
        return None;
    }
    // sqlparser gives no location for an error at the end of input
    let offset = match LOCATION.captures(&err) {
        Some(caps) => offset_of(sql, caps[1].parse().ok()?, caps[2].parse().ok()?)?,
        None if err.contains("found: EOF") => sql.trim_end().len(),
        None => return None,
    };
    let detail = LOCATION.replace(&err, "").to_string();
    let detail = detail
        .strip_prefix("sql parser error: ")
        .unwrap_or(&detail)
        .trim()
        .to_string();
    Some((offset, detail))
}

/// An error no warehouse extension and no other dialect explains.
fn certain(sql: &str, offset: usize, detail: &str) -> bool {
    if near_extension(sql, offset) {
        trace(&format!("declined: extension: {detail}"));
        return false;
    }
    if other_dialects().iter().any(|d| parses(d.as_ref(), sql)) {
        trace(&format!("declined: other dialect: {detail}"));
        return false;
    }
    true
}

fn dialect_name(dialect: &Dialect) -> &str {
    if dialect.as_str().is_empty() {
        "generic"
    } else {
        dialect.as_str()
    }
}

fn uses_is_incremental(text: &str) -> bool {
    text.contains("is_incremental")
}

/// The rendered line holding `offset` (1-based) and its text, shortened.
fn rendered_line(sql: &str, offset: usize) -> (usize, String) {
    let offset = offset.min(sql.len());
    let line = sql[..offset].matches('\n').count() + 1;
    let start = sql[..offset].rfind('\n').map_or(0, |i| i + 1);
    let end = sql[offset..].find('\n').map_or(sql.len(), |i| offset + i);
    let text = sql[start..end].trim();
    let shown: String = text.chars().take(120).collect();
    let shown = if shown.len() < text.len() {
        format!("{shown}…")
    } else {
        shown
    };
    (line, shown)
}

/// Where in the model's own text the rendered error is: the rendered
/// text around it (the token, and up to 40 bytes of its line before it)
/// found once in the template, shortened from the left until it is; the
/// first line of the model when nothing is.
fn source_span(text: &str, sql: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(sql.len());
    let token = token_end(sql, offset);
    let line_start = sql[..offset].rfind('\n').map_or(0, |i| i + 1);
    let mut from = offset.saturating_sub(40).max(line_start);
    while !sql.is_char_boundary(from) {
        from += 1;
    }
    loop {
        let needle = &sql[from..token];
        if !needle.trim().is_empty() {
            let mut found = text.match_indices(needle);
            if let (Some((at, _)), None) = (found.next(), found.next()) {
                let start = at + (offset - from);
                return (start, start + (token - offset));
            }
        }
        if from >= offset {
            break;
        }
        from += sql[from..].chars().next().map_or(1, char::len_utf8);
    }
    let first = text.len() - text.trim_start().len();
    let end = text[first..].find('\n').map_or(text.len(), |i| first + i);
    (first, end)
}

/// Why the check said nothing (or what it found), for `--debug` logs.
fn trace(what: &str) {
    tracing::trace!(target: "cogitator::syntax", "sql syntax check: {what}");
}

fn parses(dialect: &dyn SqlDialect, sql: &str) -> bool {
    Parser::parse_sql(dialect, sql).is_ok()
}

fn other_dialects() -> Vec<Box<dyn SqlDialect>> {
    vec![
        Box::new(sd::GenericDialect {}),
        Box::new(sd::DuckDbDialect {}),
        Box::new(sd::PostgreSqlDialect {}),
        Box::new(sd::SnowflakeDialect {}),
        Box::new(sd::BigQueryDialect {}),
        Box::new(sd::DatabricksDialect {}),
        Box::new(sd::RedshiftSqlDialect {}),
        Box::new(sd::MySqlDialect {}),
        Box::new(sd::MsSqlDialect {}),
        Box::new(sd::ClickHouseDialect {}),
        Box::new(sd::HiveDialect {}),
        Box::new(sd::SQLiteDialect {}),
        Box::new(sd::AnsiDialect {}),
    ]
}

/// `{% set x %}…{% endset %}`: its body is a Jinja string, not SQL.
fn has_block_set(doc: &ParsedDocument) -> bool {
    let tokens = &doc.tokens;
    tokens.iter().enumerate().any(|(i, t)| {
        if t.kind != TokenKind::JinjaOpen(JinjaKind::Stmt) {
            return false;
        }
        let body: Vec<_> = tokens[i + 1..]
            .iter()
            .take_while(|t| t.kind != TokenKind::JinjaClose(JinjaKind::Stmt))
            .collect();
        let is_set = body
            .first()
            .is_some_and(|t| &doc.text[t.start as usize..t.end as usize] == "set");
        is_set && !body.iter().any(|t| t.kind == TokenKind::Symbol('='))
    })
}

/// Byte spans of every `{{ … }}` and `{% … %}` of the template.
fn jinja_spans(doc: &ParsedDocument) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut open = None;
    for t in &doc.tokens {
        match t.kind {
            TokenKind::JinjaOpen(_) => open = Some(t.start as usize),
            TokenKind::JinjaClose(_) => {
                if let Some(start) = open.take() {
                    spans.push((start, t.end as usize));
                }
            }
            _ => {}
        }
    }
    spans
}

/// The error sits in a masked region, or the last SQL before it is one.
fn near_jinja(masked: &str, offset: usize, spans: &[(usize, usize)]) -> bool {
    let inside = |at: usize| spans.iter().any(|&(s, e)| s <= at && at < e);
    if inside(offset) {
        return true;
    }
    let before = masked.as_bytes()[..offset.min(masked.len())]
        .iter()
        .rposition(|b| !b.is_ascii_whitespace());
    before.is_some_and(inside)
}

/// Warehouse extensions sqlparser parses only in part (DuckDB's `ASOF
/// LEFT JOIN`, `POSITIONAL JOIN`, `ANTI`/`SEMI` joins, lambdas, …): an
/// error at or right after one of these words is the parser's gap, not
/// the user's mistake.
const EXTENSIONS: &[&str] = &[
    "asof",
    "positional",
    "anti",
    "semi",
    "lambda",
    "pivot",
    "unpivot",
    "match_recognize",
    "columns",
    "exclude",
    "replace",
    "rename",
    "sample",
    "tablesample",
    "summarize",
];

fn near_extension(masked: &str, offset: usize) -> bool {
    let words: Vec<String> = masked[..offset.min(masked.len())]
        .rsplit(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty())
        .take(2)
        .map(str::to_ascii_lowercase)
        .collect();
    let at = masked[offset.min(masked.len())..]
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    std::iter::once(&at)
        .chain(words.iter())
        .any(|w| EXTENSIONS.contains(&w.as_str()))
}

/// sqlparser's 1-based line and character column as a byte offset.
fn offset_of(text: &str, line: usize, column: usize) -> Option<usize> {
    let start = if line <= 1 {
        0
    } else {
        text.match_indices('\n').nth(line - 2)?.0 + 1
    };
    let rest = &text[start..];
    let within = rest
        .char_indices()
        .nth(column.saturating_sub(1))
        .map_or(rest.len(), |(at, _)| at);
    Some(start + within)
}

/// The end of the word (or the one character) at `offset`.
fn token_end(text: &str, offset: usize) -> usize {
    let rest = &text[offset.min(text.len())..];
    let word = rest
        .char_indices()
        .find(|(_, c)| !(c.is_alphanumeric() || *c == '_'))
        .map_or(rest.len(), |(at, _)| at);
    let len = if word > 0 {
        word
    } else {
        rest.chars().next().map_or(0, char::len_utf8)
    };
    offset + len
}
