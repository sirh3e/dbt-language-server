//! Macro call sites anywhere in the Jinja layer (02 §3.3 addendum): not
//! only `{{ name(` but every `name(` / `pkg.name(` inside an expression
//! or a statement — `{% set x = m() %}`, `{% if m() %}`, `{% for c in
//! m(ref('x')) %}`, `{% do m() %}`, `{% call m() %}`, nested arguments
//! and filter arguments. Pure over minijinja's token stream; what is
//! not a macro call is decided here once:
//!
//! - Jinja keywords (`if (`, `in (`, `not (`), definition names (`{%
//!   macro name(`, `{% test name(`) and the context functions dbt gives
//!   every template (`return`, `range`, `log`, `caller`, …);
//! - attribute and method calls (`x.items()`, `adapter.dispatch(`),
//!   filters (`| default(`) and tests (`is defined(`);
//! - names the template binds itself (`{% set f = … %}`, `{% for c in
//!   … %}`, macro parameters, `{% from … import f %}`), and qualified
//!   calls on them or on a context object (`this.`, `loop.`, …).

use std::collections::HashSet;

use crate::call_words::{CONTEXT_FUNCTIONS, CONTEXT_OBJECTS, KEYWORDS, VALUE_METHODS};
use crate::refine::kind_at;
use crate::{JinjaKind, Token, TokenKind};

/// The names a document binds itself: `set` and `for` targets, macro
/// parameters and imported names. A call to one is no macro call.
pub(crate) fn bound_names<'a>(text: &'a str, tokens: &[Token]) -> HashSet<&'a str> {
    let lit = |t: &Token| &text[t.start as usize..t.end as usize];
    let mut out = HashSet::new();
    for i in 0..tokens.len() {
        if tokens[i].kind != TokenKind::Ident {
            continue;
        }
        let opener = matches!(
            i.checked_sub(1).and_then(|p| kind_at(tokens, p)),
            Some(TokenKind::JinjaOpen(JinjaKind::Stmt))
        );
        match lit(&tokens[i]) {
            "set" | "for" if opener => targets(tokens, i + 1, &lit, &mut out),
            "macro" | "test" if opener => params(tokens, i + 2, &lit, &mut out),
            "import" | "as" => {
                if let Some(t) = tokens.get(i + 1).filter(|t| t.kind == TokenKind::Ident) {
                    out.insert(lit(t));
                }
            }
            _ => {}
        }
    }
    out
}

/// `a`, `a, b` up to the first non-target token.
fn targets<'a>(
    tokens: &[Token],
    mut j: usize,
    lit: &impl Fn(&Token) -> &'a str,
    out: &mut HashSet<&'a str>,
) {
    while let Some(t) = tokens.get(j).filter(|t| t.kind == TokenKind::Ident) {
        out.insert(lit(t));
        if kind_at(tokens, j + 1) != Some(TokenKind::Symbol(',')) {
            break;
        }
        j += 2;
    }
}

/// The parameter names of `name(a, b=…, c)` starting at the `(`.
fn params<'a>(
    tokens: &[Token],
    open: usize,
    lit: &impl Fn(&Token) -> &'a str,
    out: &mut HashSet<&'a str>,
) {
    if kind_at(tokens, open) != Some(TokenKind::Symbol('(')) {
        return;
    }
    let mut depth = 0i32;
    let mut expect_name = true;
    for t in tokens.iter().skip(open) {
        match t.kind {
            TokenKind::Symbol('(') => {
                depth += 1;
                if depth == 1 {
                    continue;
                }
            }
            TokenKind::Symbol(')') => {
                depth -= 1;
                if depth == 0 {
                    return;
                }
            }
            TokenKind::Symbol(',') if depth == 1 => {
                expect_name = true;
                continue;
            }
            TokenKind::Ident if depth == 1 && expect_name => {
                out.insert(lit(t));
            }
            TokenKind::JinjaClose(_) => return,
            _ => {}
        }
        expect_name = false;
    }
}

/// The edits marking the call at `i`, if `i` starts one: `[(i, Macro)]`
/// or `[(i, Package), (i + 2, Macro)]`.
pub(crate) fn macro_call_at(
    text: &str,
    tokens: &[Token],
    i: usize,
    bound: &HashSet<&str>,
) -> Vec<(usize, TokenKind)> {
    let lit = |j: usize| {
        tokens
            .get(j)
            .map(|t| &text[t.start as usize..t.end as usize])
    };
    let name = lit(i).unwrap_or_default();
    if KEYWORDS.contains(&name) || bound.contains(name) || !callable_position(text, tokens, i) {
        return Vec::new();
    }
    let qualified = kind_at(tokens, i + 1) == Some(TokenKind::Symbol('.'))
        && kind_at(tokens, i + 2) == Some(TokenKind::Ident)
        && kind_at(tokens, i + 3) == Some(TokenKind::Symbol('('));
    if qualified {
        let method = lit(i + 2).unwrap_or_default();
        if CONTEXT_OBJECTS.contains(&name) || VALUE_METHODS.contains(&method) {
            return Vec::new();
        }
        return vec![(i, TokenKind::Package), (i + 2, TokenKind::Macro)];
    }
    if kind_at(tokens, i + 1) == Some(TokenKind::Symbol('(')) && !CONTEXT_FUNCTIONS.contains(&name)
    {
        return vec![(i, TokenKind::Macro)];
    }
    Vec::new()
}

/// Whether the token at `i` may start a call: not an attribute (`.x`),
/// a filter (`| x`), a test (`is x`, `is not x`) or a definition's
/// name (`{% macro x(`).
fn callable_position(text: &str, tokens: &[Token], i: usize) -> bool {
    let lit = |j: usize| {
        tokens
            .get(j)
            .map(|t| &text[t.start as usize..t.end as usize])
    };
    let Some(prev) = i.checked_sub(1) else {
        return true;
    };
    if kind_at(tokens, prev) == Some(TokenKind::Symbol('.')) {
        return false;
    }
    match lit(prev) {
        Some("|" | "is") => false,
        Some("not") => lit(prev.wrapping_sub(1)) != Some("is"),
        Some("macro" | "test" | "materialization") => !matches!(
            prev.checked_sub(1).and_then(|p| kind_at(tokens, p)),
            Some(TokenKind::JinjaOpen(JinjaKind::Stmt))
        ),
        _ => true,
    }
}
