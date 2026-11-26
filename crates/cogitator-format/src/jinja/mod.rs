//! Jinja, read with minijinja's own tokenizer (its `unstable_machinery`):
//! splitting a document at its tags, laying a tag out again, and what a
//! statement does to the lines around it.

pub mod split;
pub mod tag;

pub use split::{split, Kind, Piece};
pub use tag::format_tag;

use crate::sql::TagRole;

/// Statements that open a block closed by `end<keyword>`.
const BLOCKS: [&str; 13] = [
    "if",
    "for",
    "macro",
    "call",
    "filter",
    "block",
    "with",
    "autoescape",
    "materialization",
    "test",
    "data_test",
    "snapshot",
    "docs",
];

/// What a piece does to the indentation of the lines after it.
pub fn role(piece: &Piece<'_>) -> TagRole {
    if piece.kind != Kind::Stmt {
        return TagRole::Other;
    }
    let keyword = piece.keyword();
    // `{% set x %}…{% endset %}` is a block, `{% set x = 1 %}` is not
    let block_set = keyword == "set"
        && !piece
            .tokens
            .iter()
            .any(|t| t.kind == split::TokKind::Assign);
    match keyword {
        "elif" | "else" => TagRole::Mid,
        k if k.starts_with("end") => TagRole::Close,
        k if BLOCKS.contains(&k) || block_set => TagRole::Open,
        _ => TagRole::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("{% if x %}", TagRole::Open)]
    #[case("{%- for c in cols %}", TagRole::Open)]
    #[case("{% macro m(a) %}", TagRole::Open)]
    #[case("{% set x %}", TagRole::Open)]
    #[case("{% set x = 1 %}", TagRole::Other)]
    #[case("{% else %}", TagRole::Mid)]
    #[case("{% elif y %}", TagRole::Mid)]
    #[case("{% endif %}", TagRole::Close)]
    #[case("{% endmacro %}", TagRole::Close)]
    #[case("{% do log('x') %}", TagRole::Other)]
    #[case("{{ x }}", TagRole::Other)]
    fn a_statements_role(#[case] tag: &str, #[case] expected: TagRole) {
        assert_eq!(role(&split(tag).unwrap()[0]), expected);
    }
}
