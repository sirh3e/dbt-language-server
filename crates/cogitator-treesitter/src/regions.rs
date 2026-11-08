//! Where the Jinja constructs are, according to the jinja2 CST (06 §3.3).
//!
//! The grammar can't end a comment at an even run of `#` (`##}`) and has no
//! `{% raw %}`. Text using either is parsed from a same-length copy with
//! those constructs blanked, and their spans are added back as regions.

use cogitator_parser::jinja_spans::{jinja_spans, JinjaSpan, JinjaSpanKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RegionKind {
    Jinja,
    Comment,
    Error,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Region {
    pub(crate) kind: RegionKind,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

/// What tree-sitter should parse for a given text.
pub(crate) struct Prepared {
    /// `None`: parse the document text as it is.
    pub(crate) normalised: Option<String>,
    /// Regions the grammar can't see in the normalised text.
    pub(crate) injected: Vec<Region>,
}

const RAW_BODY_DELIMITERS: [&str; 6] = ["{{", "}}", "{%", "%}", "{#", "#}"];

pub(crate) fn prepare(text: &str) -> Prepared {
    let spans = jinja_spans(text);
    let confuses_grammar = spans.iter().any(|s| match s.kind {
        JinjaSpanKind::RawTag => true,
        JinjaSpanKind::Comment => text[s.range.clone()].ends_with("##}"),
        _ => false,
    });
    if !confuses_grammar {
        return Prepared {
            normalised: None,
            injected: Vec::new(),
        };
    }

    let mut bytes = text.as_bytes().to_vec();
    let mut injected = Vec::new();
    for span in &spans {
        match span.kind {
            JinjaSpanKind::Comment | JinjaSpanKind::RawTag => {
                blank(&mut bytes[span.range.clone()]);
                injected.push(Region {
                    kind: RegionKind::Comment,
                    start: span.range.start,
                    end: span.range.end,
                });
            }
            JinjaSpanKind::RawBody => {
                let body = &text[span.range.clone()];
                for delimiter in RAW_BODY_DELIMITERS {
                    for (at, _) in body.match_indices(delimiter) {
                        let at = span.range.start + at;
                        blank(&mut bytes[at..at + delimiter.len()]);
                    }
                }
            }
            JinjaSpanKind::Code => {}
        }
    }
    Prepared {
        // Only ASCII delimiters and whole spans of chars were overwritten.
        normalised: String::from_utf8(bytes).ok(),
        injected,
    }
}

fn blank(bytes: &mut [u8]) {
    for b in bytes {
        if *b != b'\n' && *b != b'\r' {
            *b = b' ';
        }
    }
}

/// Top-level walk of the jinja2 CST: group `{{ … }}` token runs, take
/// `jinja_expression` ({% … %}) and comment spans, and surface ERROR spans.
/// Everything between regions is SQL text.
pub(crate) fn collect_regions(
    root: tree_sitter::Node<'_>,
    text: &str,
    injected: &[Region],
) -> Vec<Region> {
    let mut regions = injected.to_vec();
    let comments: Vec<JinjaSpan> = jinja_spans(text)
        .into_iter()
        .filter(|s| s.kind == JinjaSpanKind::Comment)
        .collect();
    let count = root.child_count();
    let mut i = 0;
    while i < count {
        let Some(node) = root.child(i) else { break };
        match node.kind() {
            "{{" => {
                // run to the matching `}}` sibling (or as far as the children go)
                let start = node.start_byte();
                let mut end = node.end_byte();
                let mut j = i + 1;
                while let Some(sibling) = root.child(j) {
                    end = sibling.end_byte();
                    if sibling.kind() == "}}" {
                        break;
                    }
                    j += 1;
                }
                regions.push(Region {
                    kind: RegionKind::Jinja,
                    start,
                    end,
                });
                i = j;
            }
            "jinja_expression" => {
                regions.push(Region {
                    kind: RegionKind::Jinja,
                    start: node.start_byte(),
                    end: node.end_byte(),
                });
            }
            "{#" => {
                // The grammar marks the comment-body token HIDDEN, so the
                // sibling walk can land on a later construct and swallow
                // it (a well-formed comment then ate the rest of the
                // document). Take the real extent from the shared spans.
                let start = node.start_byte();
                let end = comments
                    .binary_search_by_key(&start, |s| s.range.start)
                    .map_or(node.end_byte(), |at| comments[at].range.end);
                regions.push(Region {
                    kind: RegionKind::Comment,
                    start,
                    end,
                });
            }
            "ERROR" => {
                regions.push(Region {
                    kind: RegionKind::Error,
                    start: node.start_byte(),
                    end: node.end_byte(),
                });
            }
            _ => {} // anonymous text tokens → SQL gap
        }
        i += 1;
    }
    // defensive: sorted, non-overlapping
    regions.sort_by_key(|r| r.start);
    regions.dedup_by(|b, a| {
        if b.start < a.end {
            a.end = a.end.max(b.end);
            true
        } else {
            false
        }
    });
    regions
}
