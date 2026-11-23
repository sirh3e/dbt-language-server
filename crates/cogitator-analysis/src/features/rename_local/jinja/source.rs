//! dbt's block tags minijinja does not know, made parseable without
//! moving a byte: `{% test name(args) %}` / `{% endtest %}` become
//! `{%macro name(args) %}` / `{%endmacro %}` (the space before the
//! keyword gives the extra letter), so a generic test's parameters scope
//! like a macro's; the tags of `materialization`, `snapshot` and `docs`
//! blocks are blanked and their bodies stay at the top level. Found with
//! minijinja's own tokenizer.

use minijinja::machinery::{tokenize, Token};
use minijinja::syntax::SyntaxConfig;

const BLANKED: &[&str] = &[
    "materialization",
    "endmaterialization",
    "snapshot",
    "endsnapshot",
    "docs",
    "enddocs",
];

/// The text to parse: same length and line breaks as `text`.
pub(super) fn parseable(text: &str) -> Option<String> {
    let mut tokens = Vec::new();
    for t in tokenize(text, false, SyntaxConfig::default()) {
        let (token, span) = t.ok()?;
        tokens.push((token, span.start_offset as usize, span.end_offset as usize));
    }
    // (tag, keyword start, tag start, tag-open end, tag end)
    let mut tags = Vec::new();
    for (i, (token, start, open_end)) in tokens.iter().enumerate() {
        if !matches!(token, Token::BlockStart) {
            continue;
        }
        let Some((Token::Ident(tag), at, _)) = tokens.get(i + 1) else {
            continue;
        };
        let close = tokens[i + 1..]
            .iter()
            .find(|(t, ..)| matches!(t, Token::BlockEnd))
            .map_or(text.len(), |(_, _, end)| *end);
        tags.push((*tag, *at, *start, *open_end, close));
    }
    let mut out = text.as_bytes().to_vec();
    let is_test = |tag: &str| matches!(tag, "test" | "endtest");
    let rewritable = tags
        .iter()
        .filter(|(tag, ..)| is_test(tag))
        .all(|(_, at, _, open_end, _)| *at > *open_end && out[at - 1].is_ascii_whitespace());
    for &(tag, at, start, _, close) in &tags {
        match tag {
            "test" | "endtest" if rewritable => {
                let with = if tag == "test" { "macro" } else { "endmacro" };
                out[at - 1..at - 1 + with.len()].copy_from_slice(with.as_bytes());
            }
            t if is_test(t) || BLANKED.contains(&t) => blank(&mut out[start..close]),
            _ => {}
        }
    }
    String::from_utf8(out).ok()
}

fn blank(bytes: &mut [u8]) {
    for b in bytes {
        if *b != b'\n' && *b != b'\r' {
            *b = b' ';
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dbt_tags_become_parseable_in_place() {
        for (text, want) in [
            ("{% set x = 1 %}", "{% set x = 1 %}"),
            (
                "{% test t(model) %}{{ model }}{% endtest %}",
                "{%macro t(model) %}{{ model }}{%endmacro %}",
            ),
            (
                "{%- test t(m) -%}x{%- endtest -%}",
                "{%-macro t(m) -%}x{%-endmacro -%}",
            ),
            (
                "{% snapshot s %}\nselect 1\n{% endsnapshot %}",
                "                \nselect 1\n                 ",
            ),
            (
                "{% docs d %}text{% enddocs %}",
                "            text             ",
            ),
        ] {
            let got = parseable(text).unwrap();
            assert_eq!(got, want, "{text}");
            assert_eq!(got.len(), text.len());
        }
    }

    #[test]
    fn a_test_tag_without_room_is_blanked() {
        let text = "{%test t(m) %}x{% endtest %}";
        assert_eq!(parseable(text).unwrap(), "              x             ");
    }
}
