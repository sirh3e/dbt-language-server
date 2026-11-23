//! The SQL layer: the template with its Jinja masked byte for byte
//! (`cogitator_duckdb::mask_jinja`, the DuckDB check's masking and its
//! confidence rule), parsed by sqlparser in the project's dialect.
//!
//! Declines, each a source of false errors otherwise:
//! - control blocks other than config/set (`mask_jinja` says `None`),
//!   and the block form of `set`, whose body is not the model's SQL;
//! - an error inside, or right after, a masked `{{ … }}`: the expression
//!   may render to anything, not the identifier that stands in for it;
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

use super::Finding;

/// Larger templates are not checked as you type.
pub(crate) const MAX_BYTES: usize = 512 * 1024;

static LOCATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\s*at Line: (\d+), Column: (\d+)").expect("location regex compiles")
});

const SOLUTION: &str = "correct the SQL at this position; the warehouse rejects the compiled \
     model the same way. If the SQL is valid for your warehouse, turn the check off with \
     `lint: { syntax: false }` in cogitator.yml and report the case.";

/// The first certain SQL syntax error of `text`.
pub(crate) fn check(text: &str, dialect: &Dialect) -> Option<Finding> {
    if text.len() > MAX_BYTES {
        return None;
    }
    let parsed = JinjaSqlParser.parse(text, dialect).ok()?;
    if has_block_set(&parsed) {
        return None;
    }
    let Some(masked) = cogitator_duckdb::mask_jinja(&parsed) else {
        trace("declined: control flow");
        return None;
    };
    if masked.trim().is_empty() {
        return None;
    }
    let primary = sql_dialect(dialect);
    let Some(err) = Parser::parse_sql(primary.as_ref(), &masked).err() else {
        trace("parsed");
        return None;
    };
    let err = err.to_string();
    if err.contains("recursion limit") {
        return None;
    }
    // sqlparser gives no location for an error at the end of input
    let offset = match LOCATION.captures(&err) {
        Some(caps) => offset_of(&masked, caps[1].parse().ok()?, caps[2].parse().ok()?)?,
        None if err.contains("found: EOF") => masked.trim_end().len(),
        None => return None,
    };
    let jinja = jinja_spans(&parsed);
    if near_jinja(&masked, offset, &jinja) {
        trace(&format!("declined: near jinja: {err}"));
        return None;
    }
    if near_extension(&masked, offset) {
        trace(&format!("declined: extension: {err}"));
        return None;
    }
    if other_dialects().iter().any(|d| parses(d.as_ref(), &masked)) {
        trace(&format!("declined: other dialect: {err}"));
        return None;
    }
    trace(&format!("error: {err}"));
    let detail = LOCATION.replace(&err, "").to_string();
    let detail = detail
        .strip_prefix("sql parser error: ")
        .unwrap_or(&detail)
        .trim()
        .to_string();
    Some(Finding {
        start: offset,
        end: token_end(&masked, offset),
        headline: format!("SQL syntax error: {detail}"),
        reason: format!(
            "with its Jinja masked, the model is not valid {} SQL, nor valid in any other \
             dialect cogitator knows.",
            if dialect.as_str().is_empty() {
                "generic"
            } else {
                dialect.as_str()
            }
        ),
        solution: SOLUTION,
    })
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
