//! Macro call checks (`macro-arg-count`, `macro-unknown-kwarg`,
//! `macro-duplicate-kwarg`): every call the refinement marks a macro
//! call — `{{ m() }}` and calls inside blocks alike — that resolves to a
//! scanned macro (own project or an installed package) is held to that
//! macro's parameters. The call's arguments are read by minijinja's
//! expression parser; a call that does not parse, uses a splat, or
//! calls one of dbt's builtins is skipped. A body that reads `varargs`
//! takes any number of positional arguments, one that reads `kwargs`
//! any keyword.

mod judge;

use cogitator_error::guidance::Guidance;
use cogitator_lsp::{diagnostic_severity, Diagnostic, DiagnosticData, FixKind, Position, Range};
use cogitator_parser::macro_body::call_shape;
use cogitator_parser::{LineTable, TokenKind};

pub use judge::{judge, Problem};

use super::dispatch::implementation::calling_project;
use super::references::files::resolve_macro;
use super::unresolved_builtins::is_dbt_builtin;
use super::{DbtContext, Document};

pub const ARG_COUNT: &str = "macro-arg-count";
pub const UNKNOWN_KWARG: &str = "macro-unknown-kwarg";
pub const DUPLICATE_KWARG: &str = "macro-duplicate-kwarg";

pub fn check(ctx: &DbtContext, doc: &Document) -> Vec<Diagnostic> {
    let parsed = &doc.parsed;
    let tokens = &parsed.tokens;
    let table = LineTable::new(&doc.text);
    let from = calling_project(ctx, &doc.uri);
    let mut out = Vec::new();
    for (idx, token) in tokens.iter().enumerate() {
        if token.kind != TokenKind::Macro {
            continue;
        }
        let name = parsed.literal(token);
        let qualifier = idx
            .checked_sub(2)
            .filter(|&q| tokens[q].kind == TokenKind::Package)
            .filter(|&q| tokens[q + 1].kind == TokenKind::Symbol('.'))
            .map(|q| &tokens[q]);
        if qualifier.is_none() && is_dbt_builtin(name) {
            continue;
        }
        let Some(details) = resolve_macro(ctx, &from, qualifier.map(|q| parsed.literal(q)), name)
        else {
            continue;
        };
        let Some(close) = closing_paren(tokens, idx + 1) else {
            continue;
        };
        let start = qualifier.map_or(token.start, |q| q.start);
        let source = &doc.text[start as usize..tokens[close].end as usize];
        let Some(shape) = call_shape(source) else {
            continue;
        };
        let call_range = range_of(&table, token.start, token.end);
        for problem in judge(&details.params, &details.body, &shape) {
            out.push(diagnostic(
                &table,
                call_range,
                start,
                name,
                &details.params,
                problem,
            ));
        }
    }
    out
}

/// Index of the `)` matching the `(` at `open`, within one Jinja tag.
fn closing_paren(tokens: &[cogitator_parser::Token], open: usize) -> Option<usize> {
    if tokens.get(open)?.kind != TokenKind::Symbol('(') {
        return None;
    }
    let mut depth = 0i32;
    for (i, t) in tokens.iter().enumerate().skip(open) {
        match t.kind {
            TokenKind::Symbol('(') => depth += 1,
            TokenKind::Symbol(')') => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            TokenKind::JinjaClose(_) => return None,
            _ => {}
        }
    }
    None
}

fn range_of(table: &LineTable, start: u32, end: u32) -> Range {
    let at = |o: u32| {
        let (line, character) = table.line_col(o);
        Position { line, character }
    };
    Range {
        start: at(start),
        end: at(end),
    }
}

fn diagnostic(
    table: &LineTable,
    call_range: Range,
    call_start: u32,
    macro_name: &str,
    params: &[cogitator_parser::jinja_scan::MacroParam],
    problem: Problem,
) -> Diagnostic {
    let names = || {
        params
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let kwarg_range = |name: &str, offset: u32| {
        range_of(
            table,
            call_start + offset,
            call_start + offset + name.len() as u32,
        )
    };
    let (range, code, guidance) = match problem {
        Problem::TooMany { given, takes } => (
            call_range,
            ARG_COUNT,
            Guidance::new(format!(
                "{macro_name}() takes {takes} argument{}, {given} given",
                if takes == 1 { "" } else { "s" }
            ))
            .fix(format!("pass at most ({}).", names())),
        ),
        Problem::Missing { names: missing } => (
            call_range,
            ARG_COUNT,
            Guidance::new(format!(
                "{macro_name}() is missing required argument{} {}",
                if missing.len() == 1 { "" } else { "s" },
                missing
                    .iter()
                    .map(|m| format!("'{m}'"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
            .fix("pass them, or give the parameters defaults."),
        ),
        Problem::UnknownKwarg {
            name,
            offset,
            suggestions,
        } => (
            kwarg_range(&name, offset),
            UNKNOWN_KWARG,
            Guidance::new(format!("{macro_name}() has no parameter '{name}'"))
                .suggest(suggestions)
                .fix(format!("use one of ({}).", names())),
        ),
        Problem::DuplicateKwarg { name, offset } => (
            kwarg_range(&name, offset),
            DUPLICATE_KWARG,
            Guidance::new(format!("{macro_name}() gets '{name}' twice"))
                .fix("pass each argument once."),
        ),
    };
    let mut diagnostic = Diagnostic::plain(
        range,
        guidance.to_diagnostic(),
        diagnostic_severity::WARNING,
        code,
        "cogitator",
    );
    // a did-you-mean rides as typed data: the generic quickfix renames it
    if code == UNKNOWN_KWARG && !guidance.suggestions.is_empty() {
        diagnostic.data = Some(DiagnosticData {
            fix: FixKind::ReplaceToken,
            token_range: range,
            suggestions: guidance.suggestions,
            subject: None,
        });
    }
    diagnostic
}

#[cfg(test)]
#[path = "check_tests.rs"]
mod tests;
