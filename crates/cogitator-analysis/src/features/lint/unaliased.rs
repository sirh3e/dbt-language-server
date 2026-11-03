//! Bare column identifiers in a multi-relation FROM/JOIN: ambiguous the
//! day both relations grow a column of that name.

use cogitator_error::guidance::Guidance;
use cogitator_lsp::{diagnostic_severity, Diagnostic};
use cogitator_parser::{Keyword, TokenKind};

use super::super::completion::scope::ScopeIndex;
use super::super::{token_range, Document};

pub fn check(doc: &Document) -> Vec<Diagnostic> {
    if ScopeIndex::build(doc).relations().len() < 2 {
        return Vec::new();
    }
    let tokens = &doc.parsed.tokens;
    let text = &doc.parsed.text;
    let mut out = Vec::new();
    let mut in_select = false;
    let mut in_jinja = false;
    for (i, token) in tokens.iter().enumerate() {
        match token.kind {
            TokenKind::Word(Keyword::SELECT) => in_select = true,
            TokenKind::Word(Keyword::FROM) => in_select = false,
            TokenKind::JinjaOpen(_) => in_jinja = true,
            TokenKind::JinjaClose(_) => in_jinja = false,
            TokenKind::Word(Keyword::NoKeyword) if in_select && !in_jinja => {
                // qualified (a.b), function calls f(…) and aliases `as x`
                // are fine; a bare identifier is the finding
                let before_dot = text[..token.start as usize].trim_end().ends_with('.');
                let after = text[token.end as usize..].trim_start();
                let qualifies = after.starts_with('.') || after.starts_with('(');
                let aliased = i > 0 && tokens[i - 1].kind == TokenKind::Word(Keyword::AS);
                if before_dot || qualifies || aliased {
                    continue;
                }
                let name = doc.parsed.literal(token);
                out.push(Diagnostic::plain(
                    token_range(token),
                    Guidance::new(format!("bare column '{name}' in a multi-relation query"))
                        .fix("qualify it with its relation's alias.")
                        .to_diagnostic(),
                    diagnostic_severity::WARNING,
                    "lint-unaliased-column",
                    "cogitator",
                ));
            }
            _ => {}
        }
    }
    out
}
