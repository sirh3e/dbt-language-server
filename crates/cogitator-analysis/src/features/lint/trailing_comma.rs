//! A trailing comma before FROM fails outright on dialects that reject
//! it; the ones that accept it (duckdb, snowflake, bigquery) stay quiet.

use cogitator_error::guidance::Guidance;
use cogitator_lsp::{diagnostic_severity, Diagnostic};
use cogitator_parser::{Keyword, TokenKind};

use super::super::{token_range, DbtContext, Document};

const TOLERANT_DIALECTS: &[&str] = &["duckdb", "snowflake", "bigquery"];

pub fn check(ctx: &DbtContext, doc: &Document) -> Vec<Diagnostic> {
    if TOLERANT_DIALECTS.contains(&ctx.dialect.as_str()) {
        return Vec::new();
    }
    let tokens = &doc.parsed.tokens;
    let mut out = Vec::new();
    for window in tokens.windows(2) {
        let [comma, from] = window else { continue };
        if comma.kind != TokenKind::Symbol(',') || from.kind != TokenKind::Word(Keyword::FROM) {
            continue;
        }
        let dialect = if ctx.dialect.as_str().is_empty() {
            "this"
        } else {
            ctx.dialect.as_str()
        };
        out.push(Diagnostic::plain(
            token_range(comma),
            Guidance::new("trailing comma before FROM".to_string())
                .because(format!("the {dialect} dialect rejects it."))
                .fix("drop the comma.")
                .to_diagnostic(),
            diagnostic_severity::WARNING,
            "lint-trailing-comma",
            "cogitator",
        ));
    }
    out
}
