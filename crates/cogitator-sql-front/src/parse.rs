//! Statement parsing with recovery. sqlparser has none: parse the whole
//! statement first, and on failure split at paren depth 0 into CTEs and
//! the final query, parsing each on its own. A failing part is a hole.

use cogitator_dialects::Dialect;
use cogitator_ir::common::HoleKind;
use cogitator_ir::{Degradation, DegradationKind, Span};
use cogitator_parser::backend::sql_dialect;
use sqlparser::ast::{Ident, Query};
use sqlparser::dialect::Dialect as SqlDialect;
use sqlparser::keywords::Keyword;
use sqlparser::parser::Parser;
use sqlparser::tokenizer::{Token, TokenWithSpan};

use crate::caps::{depth_upper_bound, MAX_EXPR_DEPTH};
use crate::segment::{is_significant, Lexed, Positions};

#[derive(Debug)]
pub(crate) enum Part {
    Parsed(Box<Query>),
    Hole(HoleKind, Span),
}

#[derive(Debug)]
pub(crate) struct CtePart {
    pub name: Ident,
    pub columns: Vec<Ident>,
    pub body: Part,
}

#[derive(Debug)]
pub(crate) struct Parts {
    pub recursive: bool,
    pub ctes: Vec<CtePart>,
    pub body: Part,
}

#[derive(Debug)]
pub(crate) enum Outcome {
    Whole(Box<Query>),
    Parts(Parts),
    Hole(HoleKind, Span),
}

#[derive(Debug)]
pub(crate) struct Parsed {
    pub outcome: Outcome,
    pub degraded: Vec<Degradation>,
}

pub(crate) fn parse(lexed: Lexed, dialect: &Dialect, has_jinja: bool) -> Parsed {
    let dialect = sql_dialect(dialect);
    let Lexed { tokens, pos, .. } = lexed;
    let mut degraded = Vec::new();
    let tokens = first_statement(tokens, &mut degraded);
    if !tokens.iter().any(|t| is_significant(&t.token)) {
        let kind = if has_jinja {
            HoleKind::Jinja
        } else {
            HoleKind::ParseError
        };
        degraded.push(Degradation::new(
            DegradationKind::ParseHole,
            "the model has no SQL to parse",
        ));
        return Parsed {
            outcome: Outcome::Hole(kind, Span::default()),
            degraded,
        };
    }
    let depth = depth_upper_bound(&tokens);
    let (mut tokens, reason) = if depth > MAX_EXPR_DEPTH {
        (tokens, format!("expression depth {depth} exceeds {MAX_EXPR_DEPTH}"))
    } else {
        match parse_query(dialect.as_ref(), tokens) {
            Ok(query) => {
                return Parsed {
                    outcome: Outcome::Whole(query),
                    degraded,
                }
            }
            Err((tokens, err)) => (tokens, err),
        }
    };
    tokens.retain(|t| is_significant(&t.token));
    let outcome = match split_parts(tokens, &pos, dialect.as_ref(), &mut degraded) {
        Some(parts) => Outcome::Parts(parts),
        None => {
            degraded.push(Degradation::new(DegradationKind::ParseHole, reason));
            Outcome::Hole(HoleKind::ParseError, Span::default())
        }
    };
    Parsed { outcome, degraded }
}

/// The first statement; a dbt model has exactly one.
fn first_statement(
    mut tokens: Vec<TokenWithSpan>,
    degraded: &mut Vec<Degradation>,
) -> Vec<TokenWithSpan> {
    let mut depth = 0i32;
    let mut cut = None;
    for (i, tw) in tokens.iter().enumerate() {
        match tw.token {
            Token::LParen => depth += 1,
            Token::RParen => depth -= 1,
            Token::SemiColon if depth <= 0 => {
                cut = Some(i);
                break;
            }
            _ => {}
        }
    }
    let Some(cut) = cut else { return tokens };
    if tokens[cut + 1..].iter().any(|t| is_significant(&t.token)) {
        degraded.push(Degradation::new(
            DegradationKind::UnsupportedSyntax,
            "more than one statement; only the first is analysed",
        ));
    }
    tokens.truncate(cut);
    tokens
}

/// Runs `parse_query` and insists on consuming everything. Hands the
/// tokens back on failure: the split path needs them and they are not
/// cloneable cheaply.
fn parse_query(
    dialect: &dyn SqlDialect,
    tokens: Vec<TokenWithSpan>,
) -> Result<Box<Query>, (Vec<TokenWithSpan>, String)> {
    let mut parser = Parser::new(dialect).with_tokens_with_locations(tokens);
    let result = parser.parse_query();
    match result {
        Ok(query) if parser.peek_token_ref().token == Token::EOF => Ok(query),
        Ok(_) => Err((
            parser.into_tokens(),
            "trailing tokens after the query".to_string(),
        )),
        Err(err) => Err((parser.into_tokens(), err.to_string())),
    }
}

/// `tokens` are the statement's significant tokens.
fn split_parts(
    tokens: Vec<TokenWithSpan>,
    pos: &Positions,
    dialect: &dyn SqlDialect,
    degraded: &mut Vec<Degradation>,
) -> Option<Parts> {
    if !is_keyword(tokens.first()?, Keyword::WITH) {
        let body = parse_part(tokens, pos, dialect, degraded, "the query");
        return Some(Parts {
            recursive: false,
            ctes: Vec::new(),
            body,
        });
    }
    let mut at = 1;
    let recursive = tokens
        .get(at)
        .is_some_and(|t| is_keyword(t, Keyword::RECURSIVE));
    if recursive {
        at += 1;
    }
    let mut ctes = Vec::new();
    loop {
        let header = parse_cte_header(&tokens, &mut at)?;
        let close = matching_paren(&tokens, at)?;
        let body_tokens = tokens.get(at + 1..close)?.to_vec();
        let what = format!("CTE `{}`", header.name.value);
        let body = parse_part(body_tokens, pos, dialect, degraded, &what);
        ctes.push(CtePart {
            name: header.name,
            columns: header.columns,
            body,
        });
        at = close + 1;
        if tokens.get(at).is_some_and(|t| t.token == Token::Comma) {
            at += 1;
            continue;
        }
        break;
    }
    let final_tokens = tokens.get(at..)?.to_vec();
    let body = parse_part(final_tokens, pos, dialect, degraded, "the final query");
    Some(Parts {
        recursive,
        ctes,
        body,
    })
}

struct CteHeader {
    name: Ident,
    columns: Vec<Ident>,
}

/// `name [(cols)] AS [NOT] [MATERIALIZED]`, leaving `at` on the `(`.
fn parse_cte_header(tokens: &[TokenWithSpan], at: &mut usize) -> Option<CteHeader> {
    let name = word_ident(tokens.get(*at)?)?;
    *at += 1;
    let mut columns = Vec::new();
    if tokens.get(*at).is_some_and(|t| t.token == Token::LParen) {
        let close = matching_paren(tokens, *at)?;
        columns.extend(
            tokens
                .get(*at + 1..close)?
                .iter()
                .filter(|t| t.token != Token::Comma)
                .map(word_ident)
                .collect::<Option<Vec<_>>>()?,
        );
        *at = close + 1;
    }
    if !is_keyword(tokens.get(*at)?, Keyword::AS) {
        return None;
    }
    *at += 1;
    if tokens.get(*at).is_some_and(|t| is_keyword(t, Keyword::NOT)) {
        *at += 1;
    }
    if tokens
        .get(*at)
        .is_some_and(|t| is_keyword(t, Keyword::MATERIALIZED))
    {
        *at += 1;
    }
    tokens
        .get(*at)
        .is_some_and(|t| t.token == Token::LParen)
        .then_some(CteHeader { name, columns })
}

fn matching_paren(tokens: &[TokenWithSpan], open: usize) -> Option<usize> {
    let mut depth = 0i32;
    for (i, tw) in tokens.iter().enumerate().skip(open) {
        match tw.token {
            Token::LParen => depth += 1,
            Token::RParen => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

fn is_keyword(tw: &TokenWithSpan, keyword: Keyword) -> bool {
    matches!(&tw.token, Token::Word(w) if w.keyword == keyword)
}

fn word_ident(tw: &TokenWithSpan) -> Option<Ident> {
    match &tw.token {
        Token::Word(w) => Some(Ident {
            value: w.value.clone(), // boundary: owned by the AST Ident
            quote_style: w.quote_style,
            span: tw.span,
        }),
        _ => None,
    }
}

/// Parses one part; a failure or depth overflow becomes a Hole, reported.
fn parse_part(
    tokens: Vec<TokenWithSpan>,
    pos: &Positions,
    dialect: &dyn SqlDialect,
    degraded: &mut Vec<Degradation>,
    what: &str,
) -> Part {
    let span = token_range(&tokens, pos);
    if tokens.is_empty() {
        degraded.push(Degradation::new(
            DegradationKind::ParseHole,
            format!("{what} is empty"),
        ));
        return Part::Hole(HoleKind::ParseError, span);
    }
    let depth = depth_upper_bound(&tokens);
    if depth > MAX_EXPR_DEPTH {
        degraded.push(
            Degradation::new(
                DegradationKind::CapExceeded,
                format!("expression depth {depth} in {what} exceeds {MAX_EXPR_DEPTH}"),
            )
            .at(span),
        );
        return Part::Hole(HoleKind::Truncated, span);
    }
    match parse_query(dialect, tokens) {
        Ok(query) => Part::Parsed(query),
        Err((_, err)) => {
            degraded.push(
                Degradation::new(
                    DegradationKind::ParseHole,
                    format!("{what} does not parse: {err}"),
                )
                .at(span),
            );
            Part::Hole(HoleKind::ParseError, span)
        }
    }
}

fn token_range(tokens: &[TokenWithSpan], pos: &Positions) -> Span {
    let span_of = |t: &TokenWithSpan| pos.span(t.span);
    match (
        tokens.first().and_then(span_of),
        tokens.last().and_then(span_of),
    ) {
        (Some(first), Some(last)) => Span::new(first.start, last.end),
        _ => Span::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::lex;

    fn run(sql: &str) -> Parsed {
        let dialect = Dialect::new("duckdb");
        let lexed = lex(sql, &dialect).expect("lex");
        parse(lexed, &dialect, false)
    }

    #[test]
    fn a_valid_statement_parses_whole() {
        assert!(matches!(run("select 1").outcome, Outcome::Whole(_)));
        assert!(matches!(run("select 1;").outcome, Outcome::Whole(_)));
    }

    #[test]
    fn a_broken_cte_is_isolated() {
        let parsed = run("with a as (select 1), b as (select from where), c as (select 2) select * from c");
        let Outcome::Parts(parts) = parsed.outcome else {
            panic!("expected parts");
        };
        assert_eq!(parts.ctes.len(), 3);
        assert!(matches!(parts.ctes[0].body, Part::Parsed(_)));
        assert!(matches!(parts.ctes[1].body, Part::Hole(HoleKind::ParseError, _)));
        assert!(matches!(parts.body, Part::Parsed(_)));
        assert_eq!(parsed.degraded.len(), 1);
        assert_eq!(parsed.degraded[0].kind, DegradationKind::ParseHole);
    }

    #[test]
    fn multiple_statements_keep_the_first() {
        let parsed = run("select 1; select 2");
        assert!(matches!(parsed.outcome, Outcome::Whole(_)));
        assert_eq!(parsed.degraded[0].kind, DegradationKind::UnsupportedSyntax);
    }

    #[test]
    fn empty_input_is_a_hole() {
        assert!(matches!(run("   ").outcome, Outcome::Hole(HoleKind::ParseError, _)));
    }

    #[test]
    fn a_deep_chain_is_truncated_not_parsed() {
        let sql = format!("select {} from t", vec!["a"; 2000].join(" + "));
        let parsed = run(&sql);
        assert_eq!(parsed.degraded[0].kind, DegradationKind::CapExceeded);
    }
}
