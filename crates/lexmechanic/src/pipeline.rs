//! The minijinja + sqlparser tokenization pipeline (specs/rust-port/05 §4)
//! and the dbt refinement pass (02 §3 behavior).

use std::sync::Arc;

use heresy::{Error, Result};
use lexicanum::Dialect;
use minijinja::machinery::{tokenize as jinja_tokenize, Token as JToken};
use minijinja::syntax::SyntaxConfig;
use sqlparser::dialect::{
    BigQueryDialect, Dialect as SqlDialect, DuckDbDialect, GenericDialect, PostgreSqlDialect,
    SnowflakeDialect,
};
use sqlparser::keywords::Keyword;
use sqlparser::tokenizer::{Token as SToken, Tokenizer};

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
    let mut tokens = Vec::new();
    tokenize_template(text, 0, dialect, &table, &mut tokens)?;
    Ok(assemble(text, tokens))
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

/// Tokenize a template `slice` that starts at byte `base` of the document.
/// minijinja spans are slice-relative; everything emitted is document-
/// absolute. `TemplateData` runs go through sqlparser (05 §4).
pub fn tokenize_template(
    slice: &str,
    base: u32,
    dialect: &Dialect,
    table: &LineTable,
    tokens: &mut Vec<Token>,
) -> Result<()> {
    for item in jinja_tokenize(slice, false, SyntaxConfig::default()) {
        let (jtok, span) = item.map_err(|e| Error::Jinja {
            path: Default::default(),
            message: e.to_string(),
        })?;
        let (start, end) = (base + span.start_offset, base + span.end_offset);
        match jtok {
            JToken::TemplateData(_) => {
                // The SQL segment is sliced from the original text by span so
                // whitespace-control trimming cannot desynchronize offsets.
                let segment = &slice[span.start_offset as usize..span.end_offset as usize];
                tokenize_sql_segment(segment, start, dialect, table, tokens);
            }
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

fn push(tokens: &mut Vec<Token>, kind: TokenKind, start: u32, end: u32, table: &LineTable) {
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

fn sql_dialect(dialect: &Dialect) -> Box<dyn SqlDialect> {
    match dialect.as_str() {
        "snowflake" => Box::new(SnowflakeDialect {}),
        "duckdb" => Box::new(DuckDbDialect {}),
        "bigquery" => Box::new(BigQueryDialect {}),
        "postgres" => Box::new(PostgreSqlDialect {}),
        _ => Box::new(GenericDialect {}),
    }
}

/// Tokenize one SQL segment, appending document-absolute tokens.
///
/// A tokenizer error (mid-edit, e.g. unterminated string) skips the segment —
/// strictly better than failing the whole parse.
pub fn tokenize_sql_segment(
    segment: &str,
    base: u32,
    dialect: &Dialect,
    table: &LineTable,
    out: &mut Vec<Token>,
) {
    let dialect = sql_dialect(dialect);
    let mut tokenizer = Tokenizer::new(dialect.as_ref(), segment);
    let located = match tokenizer.tokenize_with_location() {
        Ok(toks) => toks,
        Err(err) => {
            tracing::trace!(%err, "sql segment tokenize failed; skipping segment");
            return;
        }
    };

    let seg_table = LineTable::new(segment);
    for tw in located {
        let (kind, len) = match &tw.token {
            SToken::Whitespace(_) | SToken::EOF => continue,
            SToken::Word(w) => {
                let quoted = w.quote_style.is_some() as u32 * 2;
                (TokenKind::Word(w.keyword), w.value.len() as u32 + quoted)
            }
            SToken::Number(s, long) => (TokenKind::Number, s.len() as u32 + *long as u32),
            SToken::SingleQuotedString(s) | SToken::DoubleQuotedString(s) => {
                (TokenKind::StringLit, s.len() as u32 + 2)
            }
            SToken::LParen => (TokenKind::Symbol('('), 1),
            SToken::RParen => (TokenKind::Symbol(')'), 1),
            SToken::Comma => (TokenKind::Symbol(','), 1),
            SToken::Period => (TokenKind::Symbol('.'), 1),
            SToken::SemiColon => (TokenKind::Symbol(';'), 1),
            other => {
                let text = other.to_string();
                let first = text.chars().next().unwrap_or(' ');
                (TokenKind::Symbol(first), text.len() as u32)
            }
        };
        // sqlparser spans are 1-based line/column within the segment
        // (line 0 marks an empty span — skip those).
        if tw.span.start.line == 0 {
            continue;
        }
        let Some(line_start) = seg_table.line_start(tw.span.start.line as usize - 1) else {
            continue;
        };
        let offset_in_segment = line_start + tw.span.start.column.saturating_sub(1) as u32;
        let start = base + offset_in_segment;
        push(out, kind, start, start + len, table);
    }
}

/// Refinement pass (02 §3.3 behavior over the crates-based stream),
/// composed from per-construct matchers: each inspects the unmodified
/// stream and yields `(index, kind)` edits; the pass applies them at the
/// end, so matchers never observe each other's effects.
pub fn refine(text: &str, tokens: &mut [Token]) {
    let edits: Vec<(usize, TokenKind)> = (0..tokens.len())
        .filter(|&i| tokens[i].kind == TokenKind::Ident)
        .flat_map(|i| refine_at(text, tokens, i))
        .collect();
    for (index, kind) in edits {
        tokens[index].kind = kind;
    }
}

fn refine_at(text: &str, tokens: &[Token], i: usize) -> Vec<(usize, TokenKind)> {
    let lit = &text[tokens[i].start as usize..tokens[i].end as usize];
    match lit {
        "ref" => call_string_arg(tokens, i, TokenKind::Ref),
        "var" => call_string_arg(tokens, i, TokenKind::Var),
        "source" => source_args(tokens, i),
        "config" => config_marker(tokens, i),
        _ => macro_markers(tokens, i),
    }
}

/// `ref('x')` / `var('x')`: the string argument gets the dbt kind.
fn call_string_arg(tokens: &[Token], i: usize, kind: TokenKind) -> Vec<(usize, TokenKind)> {
    match (kind_at(tokens, i + 1), kind_at(tokens, i + 2)) {
        (Some(TokenKind::Symbol('(')), Some(TokenKind::StringLit)) => vec![(i + 2, kind)],
        _ => Vec::new(),
    }
}

/// `source('a', 'b')`: first arg SourceName, second SourceTable.
fn source_args(tokens: &[Token], i: usize) -> Vec<(usize, TokenKind)> {
    let mut edits = call_string_arg(tokens, i, TokenKind::SourceName);
    if edits.is_empty() {
        return edits;
    }
    if matches!(kind_at(tokens, i + 3), Some(TokenKind::Symbol(',')))
        && matches!(kind_at(tokens, i + 4), Some(TokenKind::StringLit))
    {
        edits.push((i + 4, TokenKind::SourceTable));
    }
    edits
}

/// `{{ config(` — only directly after the expression opener (02 §3.2).
fn config_marker(tokens: &[Token], i: usize) -> Vec<(usize, TokenKind)> {
    match i.checked_sub(1).and_then(|p| kind_at(tokens, p)) {
        Some(TokenKind::JinjaOpen(JinjaKind::Expr)) => vec![(i, TokenKind::Config)],
        _ => Vec::new(),
    }
}

/// `{{ macro(` and `{{ pkg.macro(` — only directly after `{{` (02 §3.2).
fn macro_markers(tokens: &[Token], i: usize) -> Vec<(usize, TokenKind)> {
    let after_opener = matches!(
        i.checked_sub(1).and_then(|p| kind_at(tokens, p)),
        Some(TokenKind::JinjaOpen(JinjaKind::Expr))
    );
    if !after_opener {
        return Vec::new();
    }
    let qualified = matches!(kind_at(tokens, i + 1), Some(TokenKind::Symbol('.')))
        && matches!(kind_at(tokens, i + 2), Some(TokenKind::Ident))
        && matches!(kind_at(tokens, i + 3), Some(TokenKind::Symbol('(')));
    if qualified {
        return vec![(i, TokenKind::Package), (i + 2, TokenKind::Macro)];
    }
    match kind_at(tokens, i + 1) {
        Some(TokenKind::Symbol('(')) => vec![(i, TokenKind::Macro)],
        _ => Vec::new(),
    }
}

fn kind_at(tokens: &[Token], i: usize) -> Option<TokenKind> {
    tokens.get(i).map(|t| t.kind)
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
                    if next.kind == TokenKind::Word(Keyword::NoKeyword) {
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
                            if next.kind == TokenKind::Word(Keyword::NoKeyword) {
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
