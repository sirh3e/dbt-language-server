//! Syntax errors as you type, with no `dbt compile` first (03 §10.1):
//! the Jinja layer through minijinja's own parser, then — when the Jinja
//! is plain enough to say what SQL it renders — the SQL with the Jinja
//! masked, through sqlparser in the project's dialect.
//!
//! The confidence rule of 07 §3.2 holds throughout: a check that cannot
//! be sure DECLINES and reports nothing. A false syntax error is worse
//! than a missed one, so every unknown (a dbt-only tag, control flow that
//! changes the SQL's shape, an error next to a masked expression, SQL
//! that some other dialect accepts) is a decline. Pure over the text.

mod jinja;
mod sql;

#[cfg(test)]
mod tests;

use cogitator_dialects::Dialect;
use cogitator_error::guidance::Guidance;
use cogitator_lsp::{diagnostic_severity, Diagnostic, Position, Range};
use cogitator_parser::LineTable;

/// The diagnostic code of a Jinja syntax error.
pub const JINJA_CODE: &str = "jinja-syntax";
/// The diagnostic code of a SQL syntax error.
pub const SQL_CODE: &str = "sql-syntax";

/// One syntax problem: a byte span into the template and its guidance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Finding {
    pub start: usize,
    pub end: usize,
    pub headline: String,
    pub reason: String,
    pub solution: &'static str,
}

/// Which layers are checked: Jinja always, SQL unless another engine
/// (the DuckDB parser) already reports it for this document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntaxOptions {
    pub sql: bool,
}

impl Default for SyntaxOptions {
    fn default() -> Self {
        SyntaxOptions { sql: true }
    }
}

/// The syntax diagnostics of one `.sql` document (at most one: the
/// first Jinja error, else the first SQL error). Other files, and the
/// compiled SQL under `target/`, get none.
pub fn syntax_diagnostics(
    text: &str,
    uri: &str,
    dialect: &Dialect,
    options: SyntaxOptions,
) -> Vec<Diagnostic> {
    if !applies_to(uri) {
        return Vec::new();
    }
    if let Some(found) = jinja::check(text) {
        return vec![diagnostic(text, found, JINJA_CODE)];
    }
    if options.sql {
        if let Some(found) = sql::check(text, dialect) {
            return vec![diagnostic(text, found, SQL_CODE)];
        }
    }
    Vec::new()
}

/// A dbt SQL file, not compiled output (`target/compiled`, `target/run`).
pub fn applies_to(uri: &str) -> bool {
    if !uri.ends_with(".sql") {
        return false;
    }
    let path = cogitator_lsp::uri_to_fs_path(uri).unwrap_or_else(|| uri.to_string());
    let parts: Vec<&std::ffi::OsStr> = std::path::Path::new(&path)
        .components()
        .map(|c| c.as_os_str())
        .collect();
    !parts
        .windows(2)
        .any(|w| w[0] == "target" && (w[1] == "compiled" || w[1] == "run"))
}

fn diagnostic(text: &str, found: Finding, code: &str) -> Diagnostic {
    let table = LineTable::new(text);
    let position = |offset: usize| {
        let (line, character) = table.line_col(offset.min(text.len()) as u32);
        Position { line, character }
    };
    let message = Guidance::new(found.headline)
        .because(found.reason)
        .fix(found.solution)
        .to_message();
    Diagnostic::plain(
        Range {
            start: position(found.start),
            end: position(found.end.max(found.start)),
        },
        message,
        diagnostic_severity::ERROR,
        code,
        "cogitator",
    )
}
