//! Token refinement (02 §3.3): the pass that turns generic words into
//! dbt kinds — ref()/source()/var() arguments, config markers and
//! macro call sites — purely over the token slice.

use crate::{JinjaKind, Token, TokenKind};

/// Refinement pass (02 §3.3 behavior over the crates-based stream),
/// composed from per-construct matchers: each inspects the unmodified
/// stream and yields `(index, kind)` edits; the pass applies them at the
/// end, so matchers never observe each other's effects.
pub(crate) fn refine(text: &str, tokens: &mut [Token]) {
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
        "ref" => ref_args(tokens, i),
        "var" => call_string_arg(tokens, i, TokenKind::Var),
        "source" => source_args(tokens, i),
        "doc" => call_string_arg(tokens, i, TokenKind::Doc),
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

/// `ref('model')` / `ref('package', 'model')`: the MODEL argument is
/// the Ref; a two-arg call's first string is the Package (02 §3.3).
fn ref_args(tokens: &[Token], i: usize) -> Vec<(usize, TokenKind)> {
    let one = call_string_arg(tokens, i, TokenKind::Ref);
    if one.is_empty() {
        return one;
    }
    if matches!(kind_at(tokens, i + 3), Some(TokenKind::Symbol(',')))
        && matches!(kind_at(tokens, i + 4), Some(TokenKind::StringLit))
    {
        return vec![(i + 2, TokenKind::Package), (i + 4, TokenKind::Ref)];
    }
    one
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

pub(crate) fn kind_at(tokens: &[Token], i: usize) -> Option<TokenKind> {
    tokens.get(i).map(|t| t.kind)
}

#[cfg(test)]
mod tests {
    use crate::pipeline::parse_document;
    use crate::TokenKind;
    use cogitator_dialects::Dialect;

    fn kinds(text: &str) -> Vec<(TokenKind, String)> {
        let doc = parse_document(text, &Dialect::default()).unwrap();
        doc.tokens
            .iter()
            .map(|t| (t.kind, doc.literal(t).to_string()))
            .collect()
    }

    #[test]
    fn one_arg_ref_marks_the_model() {
        let tokens = kinds("select 1 from {{ ref('customers') }}");
        assert!(
            tokens.contains(&(TokenKind::Ref, "customers".into())),
            "{tokens:?}"
        );
    }

    #[test]
    fn doc_call_marks_the_block_name() {
        let tokens = kinds("description: '{{ doc(\"orders_status\") }}'");
        assert!(
            tokens.contains(&(TokenKind::Doc, "orders_status".into())),
            "{tokens:?}"
        );
    }

    #[test]
    fn two_arg_ref_marks_package_then_model() {
        let tokens = kinds("select 1 from {{ ref('pkg', 'customers') }}");
        assert!(
            tokens.contains(&(TokenKind::Package, "pkg".into())),
            "{tokens:?}"
        );
        assert!(
            tokens.contains(&(TokenKind::Ref, "customers".into())),
            "{tokens:?}"
        );
        // the package string must NOT be the Ref (the pre-fix bug)
        assert!(
            !tokens.contains(&(TokenKind::Ref, "pkg".into())),
            "{tokens:?}"
        );
    }

    #[test]
    fn source_marks_name_then_table() {
        let tokens = kinds("select 1 from {{ source('stripe', 'payments') }}");
        assert!(
            tokens.contains(&(TokenKind::SourceName, "stripe".into())),
            "{tokens:?}"
        );
        assert!(
            tokens.contains(&(TokenKind::SourceTable, "payments".into())),
            "{tokens:?}"
        );
    }

    #[test]
    fn qualified_macro_marks_package_and_macro() {
        let tokens = kinds("{{ dbt_utils.star(ref('x')) }}");
        assert!(
            tokens.contains(&(TokenKind::Package, "dbt_utils".into())),
            "{tokens:?}"
        );
        assert!(
            tokens.contains(&(TokenKind::Macro, "star".into())),
            "{tokens:?}"
        );
    }
}
