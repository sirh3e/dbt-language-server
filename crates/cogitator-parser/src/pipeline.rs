//! The minijinja + sqlparser tokenization pipeline (specs/rust-port/05 §4)
//! and the dbt refinement pass (02 §3 behavior).

use std::ops::Range;
use std::sync::Arc;

use cogitator_dialects::Dialect;
use cogitator_error::{Error, Result};
use minijinja::machinery::{tokenize as jinja_tokenize, Token as JToken};
use minijinja::syntax::SyntaxConfig;
use sqlparser::dialect::{
    BigQueryDialect, Dialect as SqlDialect, DuckDbDialect, GenericDialect, PostgreSqlDialect,
    SnowflakeDialect,
};
use sqlparser::keywords::Keyword;

use crate::refine::{kind_at, refine};
use crate::sql_lex::lex_document;
use crate::{DocumentParser, JinjaKind, LineTable, ParsedDocument, Token, TokenKind};

/// Phase-1 `DocumentParser` (06 §3.5): full retokenize per parse; the caller
/// keeps the last good `ParsedDocument` for malformed input.
#[derive(Debug, Default)]
pub struct JinjaSqlParser;

impl DocumentParser for JinjaSqlParser {
    fn parse(&self, text: &str, dialect: &Dialect) -> Result<ParsedDocument> {
        parse_document(text, dialect)
    }
}

pub fn parse_document(text: &str, dialect: &Dialect) -> Result<ParsedDocument> {
    let table = LineTable::new(text);
    let (mut jinja, mut segments) = (Vec::new(), Vec::new());
    tokenize_template(text, 0, &table, &mut jinja, &mut segments)?;
    Ok(assemble(
        text,
        lex_document(text, segments, jinja, dialect, &table),
    ))
}

/// Run refinement + CTE collection and build the indexed document. Shared
/// with the tree-sitter backend (06 §3.5) via [`crate::backend`].
pub fn assemble(text: &str, mut tokens: Vec<Token>) -> ParsedDocument {
    refine(text, &mut tokens);
    let def_tokens = collect_ctes(text, &tokens);
    let mut doc = ParsedDocument {
        text: Arc::from(text),
        tokens,
        def_tokens,
        ..Default::default()
    };
    doc.build_indexes();
    doc
}

/// Tokenize the Jinja layer of a template `slice` starting at byte `base`
/// of the document. minijinja spans are slice-relative; everything emitted
/// is document-absolute. `TemplateData` runs are collected in `segments`
/// for [`lex_document`] (05 §4).
pub fn tokenize_template(
    slice: &str,
    base: u32,
    table: &LineTable,
    tokens: &mut Vec<Token>,
    segments: &mut Vec<Range<usize>>,
) -> Result<()> {
    for item in jinja_tokenize(slice, false, SyntaxConfig::default()) {
        let (jtok, span) = item.map_err(|e| Error::Jinja {
            path: Default::default(),
            message: e.to_string(),
        })?;
        let (start, end) = (base + span.start_offset, base + span.end_offset);
        match jtok {
            // Taken by span so whitespace-control trimming cannot
            // desynchronize offsets.
            JToken::TemplateData(_) => segments.push(start as usize..end as usize),
            JToken::VariableStart => push(
                tokens,
                TokenKind::JinjaOpen(JinjaKind::Expr),
                start,
                end,
                table,
            ),
            JToken::VariableEnd => push(
                tokens,
                TokenKind::JinjaClose(JinjaKind::Expr),
                start,
                end,
                table,
            ),
            JToken::BlockStart => push(
                tokens,
                TokenKind::JinjaOpen(JinjaKind::Stmt),
                start,
                end,
                table,
            ),
            JToken::BlockEnd => push(
                tokens,
                TokenKind::JinjaClose(JinjaKind::Stmt),
                start,
                end,
                table,
            ),
            JToken::Ident(_) => push(tokens, TokenKind::Ident, start, end, table),
            JToken::Str(s) => {
                // Span covers the quoted literal; the refined dbt tokens must
                // cover the inner name only (02 §3.3 / hover-by-literal).
                let inner_start = start + 1;
                let inner_end = (inner_start + s.len() as u32).min(end.saturating_sub(1));
                push(
                    tokens,
                    TokenKind::StringLit,
                    inner_start,
                    inner_end.max(inner_start),
                    table,
                );
            }
            JToken::String(ref s) => {
                let inner_start = start + 1;
                let inner_end = (inner_start + s.len() as u32).min(end.saturating_sub(1));
                push(
                    tokens,
                    TokenKind::StringLit,
                    inner_start,
                    inner_end.max(inner_start),
                    table,
                );
            }
            JToken::Int(_) | JToken::Int128(_) | JToken::Float(_) => {
                push(tokens, TokenKind::Number, start, end, table)
            }
            JToken::ParenOpen => push(tokens, TokenKind::Symbol('('), start, end, table),
            JToken::ParenClose => push(tokens, TokenKind::Symbol(')'), start, end, table),
            JToken::Dot => push(tokens, TokenKind::Symbol('.'), start, end, table),
            JToken::Comma => push(tokens, TokenKind::Symbol(','), start, end, table),
            _ => push(tokens, TokenKind::Other, start, end, table),
        }
    }

    Ok(())
}

pub(crate) fn push(
    tokens: &mut Vec<Token>,
    kind: TokenKind,
    start: u32,
    end: u32,
    table: &LineTable,
) {
    if end <= start {
        return;
    }
    let (line, column) = table.line_col(start);
    tokens.push(Token {
        kind,
        start,
        end,
        line,
        column,
    });
}

pub fn sql_dialect(dialect: &Dialect) -> Box<dyn SqlDialect> {
    match dialect.as_str() {
        "snowflake" => Box::new(SnowflakeDialect {}),
        "duckdb" => Box::new(DuckDbDialect {}),
        "bigquery" => Box::new(BigQueryDialect {}),
        "postgres" => Box::new(PostgreSqlDialect {}),
        _ => Box::new(GenericDialect {}),
    }
}

/// CTE tracking (02 §3.2–3.3): `with name as ( … ), other as ( … )`.
pub fn collect_ctes(text: &str, tokens: &[Token]) -> std::collections::HashMap<Box<str>, Token> {
    let mut map = std::collections::HashMap::new();
    let lit = |t: &Token| &text[t.start as usize..t.end as usize];

    let mut active = false;
    let mut paren_count: i32 = -1;
    let mut i = 0;
    while i < tokens.len() {
        match tokens[i].kind {
            TokenKind::Word(Keyword::WITH) => {
                if let Some(next) = tokens.get(i + 1) {
                    // ANY word can name a CTE — plenty of natural names
                    // (totals, final, …) are keywords in some dialect;
                    // the `with NAME as (` position disambiguates
                    if matches!(next.kind, TokenKind::Word(_))
                        && next.kind != TokenKind::Word(Keyword::AS)
                    {
                        active = true;
                        map.insert(Box::from(lit(next)), *next);
                        let mut j = i + 1;
                        if matches!(kind_at(tokens, j + 1), Some(TokenKind::Word(Keyword::AS))) {
                            j += 1;
                        }
                        if matches!(kind_at(tokens, j + 1), Some(TokenKind::Symbol('('))) {
                            paren_count = 1;
                            j += 1;
                        }
                        i = j;
                    }
                }
            }
            TokenKind::Symbol('(') => {
                if active {
                    paren_count += 1;
                }
            }
            TokenKind::Symbol(')') if active => {
                paren_count -= 1;
                if paren_count == 0 {
                    if matches!(kind_at(tokens, i + 1), Some(TokenKind::Symbol(','))) {
                        if let Some(next) = tokens.get(i + 2) {
                            if matches!(next.kind, TokenKind::Word(_))
                                && next.kind != TokenKind::Word(Keyword::AS)
                            {
                                map.insert(Box::from(lit(next)), *next);
                                // `name as (` after the comma re-opens.
                                let mut j = i + 2;
                                if matches!(
                                    kind_at(tokens, j + 1),
                                    Some(TokenKind::Word(Keyword::AS))
                                ) {
                                    j += 1;
                                }
                                if matches!(kind_at(tokens, j + 1), Some(TokenKind::Symbol('('))) {
                                    paren_count = 1;
                                    j += 1;
                                }
                                i = j;
                            }
                        }
                    } else {
                        active = false;
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    map
}
