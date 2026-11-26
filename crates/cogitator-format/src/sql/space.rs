//! The space between two tokens, and how a token's case is written: pure
//! functions of the tokens' types in sqlfluff's tree (LT01, CP01–CP05).

use crate::config::{Case, CaseRules};

/// A token of the tree, with what the layout reads of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Leaf<'n> {
    /// sqlfluff's type: `keyword`, `naked_identifier`, `comma`, …
    pub ty: &'n str,
    pub raw: &'n str,
    /// The type of the segment the token is in: `function_name`, …
    pub parent: &'n str,
    pub is_keyword: bool,
    /// Line breaks in the source just before it.
    pub newlines_before: usize,
    /// Whether whitespace stood before it in the source.
    pub spaced: bool,
}

impl Leaf<'_> {
    pub fn is_comment(&self) -> bool {
        matches!(self.ty, "inline_comment" | "block_comment" | "comment")
    }

    /// Whether it keeps the line it was written on: a comment, a tag, or
    /// SQL kept as written.
    pub fn keeps_its_line(&self) -> bool {
        self.is_comment() || self.ty == "verbatim"
    }

    pub fn is_open(&self) -> bool {
        matches!(self.ty, "start_bracket" | "start_square_bracket" | "start_curly_bracket")
    }

    pub fn is_close(&self) -> bool {
        matches!(self.ty, "end_bracket" | "end_square_bracket" | "end_curly_bracket")
    }
}

/// `""` or `" "` between two tokens on one line.
pub fn space(prev: &Leaf<'_>, next: &Leaf<'_>) -> &'static str {
    let glued = next.ty == "comma"
        || next.ty == "statement_terminator"
        || prev.is_open()
        || next.is_close()
        || prev.ty == "dot"
        || next.ty == "dot"
        || prev.ty == "casting_operator"
        || next.ty == "casting_operator"
        || prev.ty == "sign_indicator"
        || matches!(prev.ty, "colon" | "semi_structured_op")
        || matches!(next.ty, "colon" | "semi_structured_op")
        || (next.is_open() && is_callee(prev))
        // two symbols that touched stay touching: `||`, `!=`, `->` and
        // `=>` are two tokens to some dialects, and one operator
        || (!next.spaced && is_symbol(prev.raw) && is_symbol(next.raw));
    if glued {
        ""
    } else {
        " "
    }
}

/// Punctuation, not a word, a number or a quoted thing.
pub fn is_symbol(raw: &str) -> bool {
    !raw.is_empty()
        && raw
            .chars()
            .all(|c| c.is_ascii_punctuation() && !matches!(c, '"' | '\'' | '`' | '_' | '(' | ')' | '[' | ']' | '{' | '}' | ','))
}

/// Whether a bracket after this token opens its arguments or subscript:
/// `count(`, `xs[`, `)(`.
fn is_callee(prev: &Leaf<'_>) -> bool {
    matches!(prev.ty, "function_name_identifier" | "end_square_bracket")
        || matches!(prev.parent, "function_name" | "array_accessor")
        || (prev.ty == "naked_identifier" && prev.parent == "array_type")
}

/// The token as the case rules write it.
pub fn cased(leaf: &Leaf<'_>, rules: &CaseRules, placeholder: impl Fn(&str) -> bool) -> String {
    if placeholder(leaf.raw) || leaf.raw.starts_with(['"', '`', '[', '\'']) {
        return leaf.raw.to_string();
    }
    let case = match leaf.ty {
        "null_literal" | "boolean_literal" => rules.literals,
        "function_name_identifier" => rules.functions,
        "data_type_identifier" => rules.types,
        _ if leaf.parent == "data_type" => rules.types,
        _ if leaf.is_keyword => rules.keywords,
        "naked_identifier" => rules.identifiers,
        _ => Case::Preserve,
    };
    apply(case, leaf.raw)
}

fn apply(case: Case, raw: &str) -> String {
    match case {
        Case::Upper => raw.to_uppercase(),
        Case::Lower => raw.to_lowercase(),
        Case::Preserve => raw.to_string(),
        Case::Capitalise => {
            let lower = raw.to_lowercase();
            let mut chars = lower.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect())
                .unwrap_or_default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn leaf<'a>(ty: &'a str, raw: &'a str, parent: &'a str) -> Leaf<'a> {
        Leaf {
            ty,
            raw,
            parent,
            is_keyword: ty == "keyword",
            newlines_before: 0,
            spaced: true,
        }
    }

    #[rstest]
    #[case(("function_name_identifier", "sum", "function_name"), ("start_bracket", "(", "bracketed"), "")]
    #[case(("keyword", "in", "expression"), ("start_bracket", "(", "bracketed"), " ")]
    #[case(("naked_identifier", "a", "column_reference"), ("comma", ",", "select_clause"), "")]
    #[case(("naked_identifier", "t", "column_reference"), ("dot", ".", "column_reference"), "")]
    #[case(("naked_identifier", "a", "cast"), ("casting_operator", "::", "cast"), "")]
    #[case(("start_bracket", "(", "bracketed"), ("naked_identifier", "a", "expression"), "")]
    #[case(("naked_identifier", "a", "expression"), ("end_bracket", ")", "bracketed"), "")]
    #[case(("sign_indicator", "-", "expression"), ("numeric_literal", "1", "expression"), "")]
    #[case(("keyword", "select", "select_clause"), ("naked_identifier", "a", "column_reference"), " ")]
    #[case(("naked_identifier", "a", "expression"), ("raw_comparison_operator", ">", "comparison_operator"), " ")]
    #[case(("binary_operator", "|", "binary_operator"), ("binary_operator", "|", "binary_operator"), "")]
    fn the_space_between(
        #[case] prev: (&str, &str, &str),
        #[case] next: (&str, &str, &str),
        #[case] expected: &str,
    ) {
        let (prev, mut next) = (leaf(prev.0, prev.1, prev.2), leaf(next.0, next.1, next.2));
        // as the source had them: `||` touching, everything else spaced
        next.spaced = !(is_symbol(prev.raw) && is_symbol(next.raw));
        assert_eq!(space(&prev, &next), expected);
    }

    #[rstest]
    #[case("keyword", "SeLeCt", "", "select")]
    #[case("function_name_identifier", "SUM", "function_name", "sum")]
    #[case("null_literal", "NULL", "", "null")]
    #[case("naked_identifier", "MyCol", "column_reference", "MyCol")]
    #[case("quoted_identifier", "\"MyCol\"", "column_reference", "\"MyCol\"")]
    #[case("keyword", "VARCHAR", "data_type", "varchar")]
    fn the_case_by_default(
        #[case] ty: &str,
        #[case] raw: &str,
        #[case] parent: &str,
        #[case] expected: &str,
    ) {
        let cased = cased(&leaf(ty, raw, parent), &CaseRules::default(), |_| false);
        assert_eq!(cased, expected);
    }

    #[test]
    fn capitalise_and_placeholders() {
        let rules = CaseRules {
            keywords: Case::Capitalise,
            ..CaseRules::default()
        };
        assert_eq!(cased(&leaf("keyword", "SELECT", ""), &rules, |_| false), "Select");
        let placeholder = leaf("keyword", "CGFMT_E0", "");
        assert_eq!(cased(&placeholder, &rules, |raw| raw.starts_with("CGFMT")), "CGFMT_E0");
    }
}
