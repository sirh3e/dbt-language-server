//! Grammatical cursor-context classification: pattern recognition over the
//! token stream plus the line prefix. This is what turns completion from a
//! flat list into an engine.

use std::sync::LazyLock;

use cogitator_lsp::Position;
use cogitator_parser::{Keyword, Token, TokenKind};
use regex::Regex;

use super::Document;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionContext {
    /// `alias.` or `alias.par` — columns of what the qualifier resolves to.
    MemberAccess { qualifier: String },
    /// Right after FROM/JOIN — relations belong here.
    RelationPosition,
    /// Select list, predicates, grouping/ordering — column expressions.
    ColumnPosition,
    /// The grammar admits (almost) only these words next.
    KeywordContinuation(Vec<&'static str>),
    /// No strong signal — broad write-assist set.
    General,
}

static MEMBER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([A-Za-z_][A-Za-z0-9_]*)\.[A-Za-z0-9_]*$").unwrap());

pub fn classify(doc: &Document, pos: Position, before: &str) -> CompletionContext {
    if let Some(caps) = MEMBER_RE.captures(before) {
        return CompletionContext::MemberAccess {
            qualifier: caps[1].to_string(),
        };
    }

    let Some(last) = last_significant_token(doc, pos, before) else {
        // empty document / statement start
        return CompletionContext::KeywordContinuation(vec!["select", "with"]);
    };

    match last.kind {
        TokenKind::Word(kw) => match kw {
            Keyword::FROM | Keyword::JOIN => CompletionContext::RelationPosition,
            Keyword::GROUP | Keyword::ORDER | Keyword::PARTITION => {
                CompletionContext::KeywordContinuation(vec!["by"])
            }
            Keyword::INNER | Keyword::LEFT | Keyword::RIGHT | Keyword::FULL | Keyword::CROSS => {
                CompletionContext::KeywordContinuation(vec!["join", "outer join"])
            }
            Keyword::IS => CompletionContext::KeywordContinuation(vec!["null", "not null"]),
            Keyword::SELECT
            | Keyword::WHERE
            | Keyword::ON
            | Keyword::AND
            | Keyword::OR
            | Keyword::NOT
            | Keyword::BY
            | Keyword::HAVING
            | Keyword::QUALIFY
            | Keyword::WHEN
            | Keyword::THEN
            | Keyword::ELSE
            | Keyword::DISTINCT => CompletionContext::ColumnPosition,
            _ => CompletionContext::General,
        },
        TokenKind::Symbol(',')
        | TokenKind::Symbol('=')
        | TokenKind::Symbol('<')
        | TokenKind::Symbol('>')
        | TokenKind::Symbol('(') => CompletionContext::ColumnPosition,
        _ => CompletionContext::General,
    }
}

/// The last token that ends at or before the cursor. The partial word being
/// typed is skipped (its start lies left of the cursor but it is what the
/// client is filtering on, not grammar context).
fn last_significant_token<'d>(doc: &'d Document, pos: Position, before: &str) -> Option<&'d Token> {
    let typing_word = before
        .chars()
        .rev()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .count();
    let boundary = pos.character.saturating_sub(typing_word as u32);

    doc.parsed
        .tokens
        .iter()
        .rev()
        .find(|t| t.line < pos.line || (t.line == pos.line && t.column + t.len() <= boundary))
}
