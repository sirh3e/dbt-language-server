//! What a new local name may be, and byte offsets for positions.

use cogitator_lsp::Position;
use sqlparser::keywords::{ALL_KEYWORDS, ALL_KEYWORDS_INDEX, RESERVED_FOR_TABLE_ALIAS};

/// Words Jinja reads as syntax, or names the template gets from Jinja
/// itself: a local named so would not be a plain variable.
const JINJA_WORDS: &[&str] = &[
    "and",
    "or",
    "not",
    "in",
    "is",
    "if",
    "else",
    "elif",
    "endif",
    "for",
    "endfor",
    "true",
    "false",
    "none",
    "True",
    "False",
    "None",
    "loop",
    "caller",
    "varargs",
    "kwargs",
    "self",
    "super",
    "set",
    "endset",
    "macro",
    "endmacro",
    "call",
    "endcall",
    "filter",
    "endfilter",
    "with",
    "endwith",
    "block",
    "endblock",
    "extends",
    "include",
    "import",
    "from",
    "as",
    "recursive",
    "do",
    "raw",
    "endraw",
];

fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub(crate) fn valid_jinja_name(name: &str) -> bool {
    is_identifier(name) && !JINJA_WORDS.contains(&name)
}

/// A plain SQL identifier that may stand as a table alias or CTE name.
pub(crate) fn valid_sql_name(name: &str) -> bool {
    if !is_identifier(name) {
        return false;
    }
    let upper = name.to_ascii_uppercase();
    match ALL_KEYWORDS.binary_search(&upper.as_str()) {
        Ok(i) => !RESERVED_FOR_TABLE_ALIAS.contains(&ALL_KEYWORDS_INDEX[i]),
        Err(_) => true,
    }
}

/// The byte offset of a (line, byte-column) position, clamped to the
/// line's end.
pub(crate) fn offset_of(text: &str, pos: Position) -> Option<u32> {
    let mut start = 0usize;
    for (i, line) in text.split('\n').enumerate() {
        if i as u32 == pos.line {
            let at = start + (pos.character as usize).min(line.len());
            return text.is_char_boundary(at).then_some(at as u32);
        }
        start += line.len() + 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_names() {
        for (name, jinja, sql) in [
            ("total", true, true),
            ("_x2", true, true),
            ("2x", false, false),
            ("bad name", false, false),
            ("", false, false),
            ("loop", false, true),
            ("in", false, true),
            ("select", true, false),
            ("from", false, false),
            ("join", true, false),
            ("name", true, true),
            ("naïve", false, false),
        ] {
            assert_eq!(valid_jinja_name(name), jinja, "jinja {name}");
            assert_eq!(valid_sql_name(name), sql, "sql {name}");
        }
    }

    #[test]
    fn offsets() {
        let text = "ab\ncdé\nf";
        for (line, character, want) in [
            (0, 0, Some(0)),
            (0, 9, Some(2)),
            (1, 2, Some(5)),
            (1, 3, None),
            (2, 1, Some(9)),
            (3, 0, None),
        ] {
            assert_eq!(offset_of(text, Position { line, character }), want);
        }
    }
}
