//! The one definition of where Jinja comments and raw blocks are.
//!
//! A single leftmost-match regex, so it keeps working on text minijinja
//! rejects (mid-edit, the regex fallbacks of 06 §2.4). Matching the code
//! constructs too is what makes `{{ "{#" }}` not open a comment.

use std::ops::Range;
use std::sync::LazyLock;

use regex::Regex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JinjaSpanKind {
    /// `{# … #}`, closed by the first `#}` after the opener.
    Comment,
    /// A `{% raw %}` or `{% endraw %}` tag.
    RawTag,
    /// The text between raw tags: SQL as far as dbt is concerned.
    RawBody,
    /// `{{ … }}` or `{% … %}`.
    Code,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JinjaSpan {
    pub kind: JinjaSpanKind,
    pub range: Range<usize>,
}

static SPAN_RE: LazyLock<Regex> = LazyLock::new(|| {
    const STR: &str = r#"'(?:[^'\\]|\\.)*'|"(?:[^"\\]|\\.)*""#;
    let raw_open = r"\{%[-+]?\s*raw\s*[-+]?%\}";
    let raw_close = r"\{%[-+]?\s*endraw\s*[-+]?%\}";
    Regex::new(&format!(
        r"(?s)(?P<comment>\{{#.*?(?:#\}}|\z))|(?P<open>{raw_open})(?P<body>.*?)(?P<close>{raw_close}|\z)|(?P<code>\{{\{{(?:{STR}|[^'{q}\}}]|\}}[^\}}])*(?:\}}\}}|\z)|\{{%(?:{STR}|[^'{q}%]|%[^\}}])*(?:%\}}|\z))",
        q = '"',
    ))
    .expect("span regex compiles")
});

/// Every Jinja span of `text`, in document order. Unterminated constructs
/// run to the end of the text.
pub fn jinja_spans(text: &str) -> Vec<JinjaSpan> {
    let mut spans = Vec::new();
    let mut add = |kind, m: Option<regex::Match<'_>>| {
        if let Some(m) = m.filter(|m| !m.is_empty()) {
            spans.push(JinjaSpan {
                kind,
                range: m.range(),
            });
        }
    };
    for caps in SPAN_RE.captures_iter(text) {
        add(JinjaSpanKind::Comment, caps.name("comment"));
        add(JinjaSpanKind::RawTag, caps.name("open"));
        add(JinjaSpanKind::RawBody, caps.name("body"));
        add(JinjaSpanKind::RawTag, caps.name("close"));
        add(JinjaSpanKind::Code, caps.name("code"));
    }
    spans
}

/// Blank `spans` of the given kinds in place, keeping newlines and length.
pub fn blank_spans_in(buf: &mut [u8], spans: &[JinjaSpan], kinds: &[JinjaSpanKind]) {
    for span in spans.iter().filter(|s| kinds.contains(&s.kind)) {
        for b in &mut buf[span.range.clone()] {
            if *b != b'\n' {
                *b = b' ';
            }
        }
    }
}

/// A copy of `text` with its Jinja spans of the given kinds blanked.
pub fn blanked(text: &str, kinds: &[JinjaSpanKind]) -> String {
    let mut buf = text.as_bytes().to_vec();
    blank_spans_in(&mut buf, &jinja_spans(text), kinds);
    // Only whole spans of chars are overwritten, with ASCII.
    String::from_utf8(buf).unwrap_or_else(|_| text.to_string())
}

/// What isn't live code: comments and everything in a raw block.
pub const NOT_CODE: &[JinjaSpanKind] = &[
    JinjaSpanKind::Comment,
    JinjaSpanKind::RawTag,
    JinjaSpanKind::RawBody,
];

/// What SQL masks remove: raw bodies stay, because they render as SQL.
pub const NOT_SQL: &[JinjaSpanKind] = &[JinjaSpanKind::Comment, JinjaSpanKind::RawTag];
