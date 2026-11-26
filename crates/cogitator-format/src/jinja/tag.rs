//! One tag laid out again from its tokens: `{{ref( 'x' )}}` → `{{ ref('x') }}`.
//!
//! Only spacing changes, never a token: the result is tokenized again and
//! kept only when minijinja reads the same tokens, so a rule here can make
//! a tag look worse but never mean something else. A tag written over
//! several lines is somebody's layout and is kept as written.

use super::split::{split, Kind, Piece, Tok, TokKind};
use crate::config::JinjaRules;

/// Words after which `-`, `+`, `*` and `**` are unary, and a bracket is
/// no call (`return` is no such word: dbt calls `return(…)`).
const PREFIX_WORDS: [&str; 8] = ["and", "or", "not", "in", "is", "if", "else", "elif"];

/// The tag as it will be written.
pub fn format_tag(piece: &Piece<'_>, rules: &JinjaRules) -> String {
    if !matches!(piece.kind, Kind::Expr | Kind::Stmt) || piece.text.contains('\n') {
        return piece.text.to_string();
    }
    let body = if rules.normalize_tags {
        join(&piece.tokens, rules)
    } else {
        inner(piece).trim().to_string()
    };
    let written = if rules.delimiter_spacing || rules.normalize_tags {
        let pad = if rules.delimiter_spacing { " " } else { "" };
        if body.is_empty() {
            format!("{}{pad}{}", piece.open, piece.close)
        } else {
            format!("{}{pad}{body}{pad}{}", piece.open, piece.close)
        }
    } else {
        piece.text.to_string()
    };
    if same_tokens(piece.text, &written) {
        written
    } else {
        piece.text.to_string()
    }
}

fn inner<'a>(piece: &Piece<'a>) -> &'a str {
    &piece.text[piece.open.len()..piece.text.len() - piece.close.len()]
}

/// The tokens with the spaces the rules put between them.
fn join(tokens: &[Tok<'_>], rules: &JinjaRules) -> String {
    let mut out = String::new();
    // the bracket each open one is, to tell `f(a=1)` from `{% set a = 1 %}`
    let mut brackets: Vec<&str> = Vec::new();
    for (i, tok) in tokens.iter().enumerate() {
        let prev = i.checked_sub(1).map(|p| &tokens[p]);
        let before = i.checked_sub(2).map(|p| &tokens[p]);
        if let Some(prev) = prev {
            if space_between(before, prev, tok, brackets.last().copied(), rules) {
                out.push(' ');
            }
        }
        out.push_str(tok.text);
        match tok.kind {
            TokKind::Open => brackets.push(tok.text),
            TokKind::Close => {
                brackets.pop();
            }
            _ => {}
        }
    }
    out
}

/// Whether one space goes between `prev` and `next` (`before` is the token
/// before `prev`, `bracket` the innermost open bracket).
fn space_between(
    before: Option<&Tok<'_>>,
    prev: &Tok<'_>,
    next: &Tok<'_>,
    bracket: Option<&str>,
    rules: &JinjaRules,
) -> bool {
    use TokKind::*;
    let unary_prev = prev.kind == Sign && is_prefix_position(before);
    match (prev.kind, next.kind) {
        (Open, _) | (_, Close) | (_, Comma) | (Dot, _) | (_, Dot) => false,
        (_, Colon) => false,
        (Colon, _) => bracket != Some("["),
        // a call or a subscript: `f(`, `xs[`, `)(`; but `in (`, `not (`
        (_, Open) if next.text != "{" => match prev.kind {
            Ident => PREFIX_WORDS.contains(&prev.text),
            Str | Close => false,
            _ => true,
        },
        (Pipe, _) | (_, Pipe) => rules.filter_spacing,
        (Assign, _) | (_, Assign) => bracket != Some("("),
        _ if unary_prev => false,
        _ => true,
    }
}

/// Whether a sign after `before` is unary: at the start, after an
/// operator, an open bracket, a comma or one of the PREFIX_WORDS.
fn is_prefix_position(before: Option<&Tok<'_>>) -> bool {
    before.is_none_or(|tok| match tok.kind {
        TokKind::Ident => PREFIX_WORDS.contains(&tok.text),
        TokKind::Str | TokKind::Number | TokKind::Close => false,
        _ => true,
    })
}

/// Whether two tags read as the same tokens.
fn same_tokens(a: &str, b: &str) -> bool {
    let texts = |s: &str| -> Option<Vec<String>> {
        let pieces = split(s).ok()?;
        let tag = pieces.first().filter(|_| pieces.len() == 1)?;
        Some(
            std::iter::once(tag.open.trim().to_string())
                .chain(tag.tokens.iter().map(|t| t.text.to_string()))
                .chain(std::iter::once(tag.close.trim().to_string()))
                .collect(),
        )
    };
    texts(a).is_some_and(|ta| texts(b) == Some(ta))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn fmt(tag: &str) -> String {
        let pieces = split(tag).unwrap();
        format_tag(&pieces[0], &JinjaRules::default())
    }

    #[rstest]
    #[case("{{ref( 'x' )}}", "{{ ref('x') }}")]
    #[case("{{ ref('p','m', v=2) }}", "{{ ref('p', 'm', v=2) }}")]
    #[case("{{x|upper}}", "{{ x | upper }}")]
    #[case("{{ a.b . c }}", "{{ a.b.c }}")]
    #[case("{%set x=-1%}", "{% set x = -1 %}")]
    #[case("{% if a  and  not b %}", "{% if a and not b %}")]
    #[case("{{ x - 1 }}", "{{ x - 1 }}")]
    #[case("{{ f(**kw) }}", "{{ f(**kw) }}")]
    #[case("{{ {'a':1} }}", "{{ {'a': 1} }}")]
    #[case("{{ xs[1:2] }}", "{{ xs[1:2] }}")]
    #[case("{{ xs [0] }}", "{{ xs[0] }}")]
    #[case("{%- for c in cols -%}", "{%- for c in cols -%}")]
    #[case("{{ 'a' ~ b }}", "{{ 'a' ~ b }}")]
    #[case("{{x}}", "{{ x }}")]
    #[case("{% endif %}", "{% endif %}")]
    #[case("{{ x in (1, 2) }}", "{{ x in (1, 2) }}")]
    #[case("{{ return([]) }}", "{{ return([]) }}")]
    fn a_tag_laid_out(#[case] tag: &str, #[case] expected: &str) {
        assert_eq!(fmt(tag), expected);
    }

    #[test]
    fn several_lines_are_kept() {
        let tag = "{{ config(\n    materialized='table'\n) }}";
        assert_eq!(fmt(tag), tag);
    }

    #[test]
    fn rules_off_keep_the_inside() {
        let rules = JinjaRules {
            normalize_tags: false,
            delimiter_spacing: false,
            ..JinjaRules::default()
        };
        let pieces = split("{{ref( 'x' )}}").unwrap();
        assert_eq!(format_tag(&pieces[0], &rules), "{{ref( 'x' )}}");
    }
}
