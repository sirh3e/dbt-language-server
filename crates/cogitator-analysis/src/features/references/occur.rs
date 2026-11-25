//! Occurrence finders: pure functions from a parsed document (or a file
//! text) to the ranges where a dbt name is used or defined.

use cogitator_lsp::{Position, Range};
use cogitator_parser::jinja_scan::{self, RawMacro};
use cogitator_parser::{ParsedDocument, TokenKind};

use super::super::token_range;

/// The package of a qualified call `pkg.name(`: the Package token two
/// before the Macro token, with a `.` between them.
pub(crate) fn qualifier(parsed: &ParsedDocument, idx: usize) -> Option<&str> {
    let dot = idx.checked_sub(1).and_then(|i| parsed.tokens.get(i))?;
    let package = idx.checked_sub(2).and_then(|i| parsed.tokens.get(i))?;
    (dot.kind == TokenKind::Symbol('.') && package.kind == TokenKind::Package)
        .then(|| parsed.literal(package))
}

/// Every macro call of a document: token index, qualifier, name.
pub(crate) fn macro_calls(parsed: &ParsedDocument) -> Vec<(usize, Option<&str>, &str)> {
    parsed
        .tokens
        .iter()
        .enumerate()
        .filter(|(_, t)| t.kind == TokenKind::Macro)
        .map(|(i, t)| (i, qualifier(parsed, i), parsed.literal(t)))
        .collect()
}

/// `source('source')` uses (the name token) or, with a table,
/// `source('source', 'table')` uses (the table token).
pub(crate) fn source_uses(
    parsed: &ParsedDocument,
    source: &str,
    table: Option<&str>,
) -> Vec<Range> {
    parsed
        .tokens
        .iter()
        .enumerate()
        .filter(|(i, t)| match (t.kind, table) {
            (TokenKind::SourceName, None) => parsed.literal(t) == source,
            (TokenKind::SourceTable, Some(table)) => {
                parsed.literal(t) == table
                    && parsed
                        .lookback_find(*i, TokenKind::SourceName, 4)
                        .is_some_and(|s| parsed.literal(s) == source)
            }
            _ => false,
        })
        .map(|(_, t)| token_range(t))
        .collect()
}

/// `doc('name')` uses.
pub(crate) fn doc_uses(parsed: &ParsedDocument, name: &str) -> Vec<Range> {
    parsed
        .tokens
        .iter()
        .filter(|t| t.kind == TokenKind::Doc && parsed.literal(t) == name)
        .map(token_range)
        .collect()
}

/// `function('name')` uses.
pub(crate) fn function_uses(parsed: &ParsedDocument, name: &str) -> Vec<Range> {
    parsed
        .tokens
        .iter()
        .filter(|t| t.kind == TokenKind::Function && parsed.literal(t) == name)
        .map(token_range)
        .collect()
}

/// An identifier without its SQL quotes.
pub(crate) fn unquoted(word: &str) -> &str {
    word.trim_matches(|c| c == '"' || c == '`' || c == '[' || c == ']')
}

/// SQL words spelling `column` (case-insensitive, quotes ignored).
pub(crate) fn word_uses(parsed: &ParsedDocument, column: &str) -> Vec<Range> {
    parsed
        .tokens
        .iter()
        .filter(|t| matches!(t.kind, TokenKind::Word(_) | TokenKind::Ident))
        .filter(|t| unquoted(parsed.literal(t)).eq_ignore_ascii_case(column))
        .map(token_range)
        .collect()
}

/// The range `name` occupies at `pos` in `text`, one opening quote
/// skipped — `None` when the text there is not the name (a stale side
/// table, an escaped scalar).
pub(crate) fn name_span(text: &str, pos: Position, name: &str) -> Option<Range> {
    let line = text.lines().nth(pos.line as usize)?;
    let mut start = pos.character as usize;
    if line[start.min(line.len())..].starts_with(['\'', '"']) {
        start += 1;
    }
    let rest = line.get(start..)?;
    let after = rest.get(name.len()..)?;
    let boundary = !after.starts_with(|c: char| c.is_alphanumeric() || c == '_');
    (rest.starts_with(name) && boundary).then(|| span(pos.line, start as u32, name.len() as u32))
}

pub(crate) fn span(line: u32, character: u32, len: u32) -> Range {
    Range {
        start: Position { line, character },
        end: Position {
            line,
            character: character + len,
        },
    }
}

pub(crate) fn contains(range: &Range, pos: Position) -> bool {
    let after_start = (pos.line, pos.character) >= (range.start.line, range.start.character);
    let before_end = (pos.line, pos.character) <= (range.end.line, range.end.character);
    after_start && before_end
}

/// The name of a `{% macro name(…) %}` header.
pub(crate) fn macro_name_range(raw: &RawMacro) -> Range {
    span(raw.start.0, raw.start.1, raw.name.len() as u32)
}

/// The macro whose definition header names it under the cursor.
pub(crate) fn macro_def_at(text: &str, pos: Position) -> Option<RawMacro> {
    jinja_scan::extract_macros(text)
        .into_iter()
        .find(|raw| contains(&macro_name_range(raw), pos))
}

/// The `{% docs name %}` block whose name is under the cursor.
pub(crate) fn docs_def_at(text: &str, pos: Position) -> Option<String> {
    jinja_scan::extract_docs_blocks(text)
        .into_iter()
        .find(|b| contains(&span(b.start.0, b.start.1, b.name.len() as u32), pos))
        .map(|b| b.name)
}

/// The token-index window of one macro's body: from its header to the
/// next macro header of the file (macros do not nest in dbt).
pub(crate) fn macro_body(
    parsed: &ParsedDocument,
    macros: &[RawMacro],
    name: &str,
) -> Option<std::ops::Range<usize>> {
    let at = macros.iter().position(|m| m.name == name)?;
    let after = |p: (u32, u32)| {
        parsed
            .tokens
            .iter()
            .position(|t| (t.line, t.column) >= p)
            .unwrap_or(parsed.tokens.len())
    };
    let start = after(macros[at].start);
    let end = macros
        .get(at + 1)
        .map_or(parsed.tokens.len(), |m| after(m.start));
    Some(start..end)
}

/// Which of a file's macros a token at `(line, column)` sits in.
pub(crate) fn enclosing_macro(macros: &[RawMacro], at: (u32, u32)) -> Option<&RawMacro> {
    macros.iter().rev().find(|m| m.start <= at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cogitator_dialects::Dialect;
    use cogitator_parser::{DocumentParser, JinjaSqlParser};

    fn parse(text: &str) -> ParsedDocument {
        JinjaSqlParser.parse(text, &Dialect::default()).unwrap()
    }

    #[test]
    fn macro_calls_carry_their_qualifier() {
        let doc = parse("select {{ a() }}, {{ pkg.b(1) }}, {{ c.d }}");
        let calls: Vec<(Option<&str>, &str)> = macro_calls(&doc)
            .into_iter()
            .map(|(_, q, n)| (q, n))
            .collect();
        assert_eq!(calls, vec![(None, "a"), (Some("pkg"), "b")]);
    }

    #[test]
    fn source_uses_by_name_or_table() {
        let doc = parse("{{ source('s', 't') }} {{ source('s', 'u') }} {{ source('x', 't') }}");
        for (table, expected) in [(None, 2), (Some("t"), 1), (Some("u"), 1), (Some("v"), 0)] {
            assert_eq!(source_uses(&doc, "s", table).len(), expected, "{table:?}");
        }
    }

    #[test]
    fn name_span_cases() {
        let pos = |line, character| Position { line, character };
        for (text, at, name, expected) in [
            ("  - name: orders\n", pos(0, 10), "orders", Some((10, 16))),
            ("  - name: 'orders'\n", pos(0, 10), "orders", Some((11, 17))),
            (
                "  - name: \"orders\"\n",
                pos(0, 10),
                "orders",
                Some((11, 17)),
            ),
            ("  - name: orders_v2\n", pos(0, 10), "orders", None),
            ("  - name: other\n", pos(0, 10), "orders", None),
            ("a\n  - name: x\n", pos(1, 10), "x", Some((10, 11))),
            ("short\n", pos(4, 0), "x", None),
        ] {
            let got = name_span(text, at, name).map(|r| (r.start.character, r.end.character));
            assert_eq!(got, expected, "{text:?}");
        }
    }

    #[test]
    fn definitions_under_the_cursor() {
        let text = "{% macro one(a) %}{{ two() }}{% endmacro %}\n{% macro two() %}x{% endmacro %}";
        let at = |line, character| Position { line, character };
        for (pos, expected) in [
            (at(0, 9), Some("one")),
            (at(0, 12), Some("one")),
            (at(1, 10), Some("two")),
            (at(0, 2), None),
        ] {
            assert_eq!(
                macro_def_at(text, pos).map(|m| m.name).as_deref(),
                expected,
                "{pos:?}"
            );
        }
        let docs = "{% docs orders_status %}\ntext\n{% enddocs %}";
        assert_eq!(
            docs_def_at(docs, at(0, 10)).as_deref(),
            Some("orders_status")
        );
        assert_eq!(docs_def_at(docs, at(1, 1)), None);
    }

    #[test]
    fn macro_bodies_split_at_the_next_header() {
        let text = "{% macro one() %}{{ two() }}{% endmacro %}\n{% macro two() %}{{ three() }}{% endmacro %}";
        let doc = parse(text);
        let macros = jinja_scan::extract_macros(text);
        let calls = |name| {
            let body = macro_body(&doc, &macros, name).unwrap();
            macro_calls(&doc)
                .into_iter()
                .filter(|(i, _, _)| body.contains(i))
                .map(|(_, _, n)| n)
                .collect::<Vec<_>>()
        };
        assert_eq!(calls("one"), vec!["two"]);
        assert_eq!(calls("two"), vec!["three"]);
        assert!(macro_body(&doc, &macros, "nope").is_none());
        assert_eq!(enclosing_macro(&macros, (1, 20)).unwrap().name, "two");
        assert_eq!(enclosing_macro(&macros, (0, 20)).unwrap().name, "one");
    }

    #[test]
    fn word_uses_ignore_case_and_quotes() {
        let doc = parse("select Amount, \"amount\", amounts from t");
        assert_eq!(word_uses(&doc, "amount").len(), 2);
    }
}
