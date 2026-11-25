//! The Jinja layer: minijinja's tokenizer and parser over the template,
//! dbt's own block tags made parseable in place
//! (`rename_local::jinja::parseable`). What minijinja rejects but dbt's
//! Jinja may accept — a statement minijinja does not know (`break`, an
//! extension's tag), a name it reserves — is a decline, not an error.

use minijinja::machinery::{parse, tokenize, Token};
use minijinja::syntax::SyntaxConfig;

use super::Finding;
use crate::features::rename_local::jinja::parseable;

/// Block tags and the tag that closes them; `set` only in its block
/// form (`{% set x %}…{% endset %}`).
const BLOCKS: &[(&str, &str)] = &[
    ("if", "endif"),
    ("for", "endfor"),
    ("macro", "endmacro"),
    ("call", "endcall"),
    ("filter", "endfilter"),
    ("with", "endwith"),
    ("autoescape", "endautoescape"),
    ("block", "endblock"),
    ("set", "endset"),
    ("test", "endtest"),
    ("snapshot", "endsnapshot"),
    ("materialization", "endmaterialization"),
    ("docs", "enddocs"),
];

const CLOSE_ALL: &str =
    "close every `{% if %}`, `{% for %}`, `{% macro %}`… with its `{% end… %}` \
     tag, and every `{{`, `{%`, `{#` with `}}`, `%}`, `#}`.";
const FIX_EXPR: &str = "correct the Jinja at this position; dbt renders the file with Jinja and \
     fails on it the same way.";
const STRAY: &str = "remove the tag, or add the block it closes before it.";

/// The first certain Jinja syntax error of `text`.
pub(crate) fn check(text: &str) -> Option<Finding> {
    let mut tokens = Vec::new();
    for item in tokenize(text, false, SyntaxConfig::default()) {
        match item {
            Ok((token, span)) => {
                tokens.push((token, span.start_offset as usize, span.end_offset as usize))
            }
            // the tokenizer only fails on what no Jinja accepts
            Err(err) => return Some(lexer_finding(text, &err)),
        }
    }
    let blocks = open_blocks(&tokens);
    if let Some(found) = blocks.stray {
        return Some(found);
    }
    let source = parseable(text)?;
    let err = parse(&source, "<syntax>", SyntaxConfig::default()).err()?;
    let detail = err.detail().unwrap_or("syntax error").to_string();
    if !certain(&detail) {
        return None;
    }
    if detail.contains("end of input") {
        if let Some((tag, start, end)) = blocks.unclosed.last() {
            let close = BLOCKS
                .iter()
                .find(|(open, _)| open == tag)
                .map_or("end", |(_, close)| close);
            return Some(Finding {
                start: *start,
                end: *end,
                headline: format!("Jinja syntax error: `{{% {tag} %}}` is never closed"),
                reason: format!("the file ends before its `{{% {close} %}}`."),
                solution: CLOSE_ALL,
            });
        }
    }
    let (start, end) = span_of(text, &err);
    Some(Finding {
        start,
        end,
        headline: format!("Jinja syntax error: {detail}"),
        reason: "minijinja cannot parse the template here, and neither can dbt's Jinja."
            .to_string(),
        solution: if detail.contains("end of input") {
            CLOSE_ALL
        } else {
            FIX_EXPR
        },
    })
}

/// Parser errors dbt's Jinja certainly shares. `unknown statement x`
/// (loop controls, extensions), reserved names and minijinja's own
/// limits are not among them.
fn certain(detail: &str) -> bool {
    detail.starts_with("unexpected")
}

fn lexer_finding(text: &str, err: &minijinja::Error) -> Finding {
    let detail = err.detail().unwrap_or("syntax error").to_string();
    let (mut start, mut end) = span_of(text, err);
    let eof = detail.contains("end of");
    // an unclosed comment, string or raw block is best shown at its opener
    let before = &text[..start.min(text.len())];
    let opener = if detail.contains("comment") {
        before.rfind("{#").map(|at| (at, 2))
    } else if detail.contains("raw block") {
        before.rfind("{%").map(|at| (at, 2))
    } else if detail.contains("string") {
        before.rfind(['\'', '"']).map(|at| (at, 1))
    } else {
        None
    };
    if let Some((at, len)) = opener {
        (start, end) = (at, at + len);
    }
    Finding {
        start,
        end,
        headline: format!("Jinja syntax error: {detail}"),
        reason: if eof {
            "a string, comment or raw block runs to the end of the file.".to_string()
        } else {
            "the Jinja tokenizer rejects this input.".to_string()
        },
        solution: if eof { CLOSE_ALL } else { FIX_EXPR },
    }
}

/// minijinja's error span, or its line, or the end of the text.
fn span_of(text: &str, err: &minijinja::Error) -> (usize, usize) {
    if let Some(range) = err.range() {
        let start = range.start.min(text.len());
        return (start, range.end.clamp(start, text.len()));
    }
    match err.line() {
        Some(line) => {
            let start = text
                .split_inclusive('\n')
                .take(line.saturating_sub(1))
                .map(str::len)
                .sum::<usize>()
                .min(text.len());
            let end = text[start..].find('\n').map_or(text.len(), |n| start + n);
            (start, end)
        }
        None => (text.len(), text.len()),
    }
}

#[derive(Default)]
struct Blocks<'a> {
    /// (tag, tag start, tag end), innermost last.
    unclosed: Vec<(&'a str, usize, usize)>,
    stray: Option<Finding>,
}

/// The block stack by tag name alone: a closing tag that matches no
/// open block is certain; the innermost unclosed opener locates an
/// end-of-input error better than minijinja's span does.
fn open_blocks<'a>(tokens: &[(Token<'a>, usize, usize)]) -> Blocks<'a> {
    let mut blocks = Blocks::default();
    for (i, (token, start, _)) in tokens.iter().enumerate() {
        if !matches!(token, Token::BlockStart) {
            continue;
        }
        let Some((Token::Ident(tag), ..)) = tokens.get(i + 1) else {
            continue;
        };
        let close_at = tokens[i + 1..]
            .iter()
            .position(|(t, ..)| matches!(t, Token::BlockEnd))
            .map(|off| i + 1 + off);
        let end = close_at.map_or(*start, |at| tokens[at].2);
        let body = close_at.map_or(&tokens[i + 1..], |at| &tokens[i + 1..at]);
        let opens = BLOCKS.iter().any(|(open, _)| open == tag)
            && (*tag != "set" || !body.iter().any(|(t, ..)| matches!(t, Token::Assign)));
        if opens {
            blocks.unclosed.push((tag, *start, end));
            continue;
        }
        let Some((open, _)) = BLOCKS.iter().find(|(_, close)| close == tag) else {
            continue;
        };
        match blocks.unclosed.iter().rposition(|(t, ..)| t == open) {
            Some(at) => blocks.unclosed.truncate(at),
            None if blocks.stray.is_none() => {
                blocks.stray = Some(Finding {
                    start: *start,
                    end,
                    headline: format!("Jinja syntax error: `{{% {tag} %}}` closes no block"),
                    reason: format!("no `{{% {open} %}}` is open at this point."),
                    solution: STRAY,
                });
            }
            None => {}
        }
    }
    blocks
}
