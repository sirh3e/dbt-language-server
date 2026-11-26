//! sqlfluff's tree as a flat run of events: its tokens, its Indent and
//! Dedent points (where sqlfluff's own reflow may break a line) and where
//! each segment starts and ends. Whitespace is not kept, only how many line
//! breaks stood before each token.

use sqlfluffrs_parser::parser::{MetaType, Node};

use super::space::Leaf;
use crate::config::{Config, SelectTargets};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event<'n> {
    Leaf(Leaf<'n>),
    /// `implicit`: sqlfluff's "the line may go on after it"; `forced`: the
    /// group it opens breaks whatever its width (LT09).
    Indent { implicit: bool, forced: bool },
    Dedent,
    Enter(&'n str),
    Exit(&'n str),
}

/// The events of a tree, in order.
pub fn events<'n>(tree: &'n Node, config: &Config) -> Vec<Event<'n>> {
    let mut out = Vec::new();
    let mut gap = Gap::default();
    walk(tree, "", config, &mut out, &mut gap);
    indents_before_comments(out)
}

/// sqlfluff puts a comment on a line of its own before the Indent of the
/// lines it starts (`case`, then `-- note`, then the Indent of `when …`):
/// the Indent goes first, so the comment is indented as the lines after it.
/// It moves no token.
fn indents_before_comments(events: Vec<Event<'_>>) -> Vec<Event<'_>> {
    let mut out: Vec<Event<'_>> = Vec::with_capacity(events.len());
    for event in events {
        let indent = matches!(event, Event::Indent { .. });
        // the own-line comments just before it, and where they start
        let start = out
            .iter()
            .rposition(|e| !matches!(e, Event::Leaf(l) if l.keeps_its_line() && l.newlines_before > 0))
            .map_or(0, |at| at + 1);
        if indent && start < out.len() {
            out.insert(start, event);
        } else {
            out.push(event);
        }
    }
    out
}

/// A token that stands for a Jinja tag, after `newlines` line breaks.
pub fn tag_leaf(raw: &str, newlines: usize) -> Event<'_> {
    Event::Leaf(Leaf {
        ty: "block_comment",
        raw,
        parent: "",
        is_keyword: false,
        newlines_before: newlines,
        spaced: true,
    })
}

/// SQL the parser could not read, by its placeholder, kept as written: its
/// first line is indented as the layout says, the others keep their own.
pub fn verbatim_leaf(placeholder: &str, newlines: usize) -> Event<'_> {
    Event::Leaf(Leaf {
        ty: "verbatim",
        raw: placeholder,
        parent: "",
        is_keyword: false,
        newlines_before: newlines,
        spaced: true,
    })
}

/// The whitespace since the last token.
#[derive(Default)]
struct Gap {
    newlines: usize,
    spaced: bool,
}

fn walk<'n>(
    node: &'n Node,
    parent: &'n str,
    config: &Config,
    out: &mut Vec<Event<'n>>,
    newlines: &mut Gap,
) {
    match node {
        Node::Raw {
            segment_type,
            raw,
            class_types,
            ..
        } => {
            if raw.trim().is_empty() {
                newlines.newlines += raw.matches('\n').count();
                newlines.spaced |= !raw.is_empty();
                return;
            }
            let gap = std::mem::take(newlines);
            out.push(Event::Leaf(Leaf {
                ty: segment_type,
                raw,
                parent,
                is_keyword: segment_type == "keyword" || class_types.iter().any(|t| t == "keyword"),
                newlines_before: gap.newlines,
                spaced: gap.spaced,
            }));
        }
        Node::Segment {
            segment_type,
            children,
            ..
        } => {
            let ty = segment_type.as_deref().unwrap_or("");
            let mut force = ty == "select_clause"
                && config.layout.select_targets == SelectTargets::OnePerLine
                && children
                    .iter()
                    .filter(|c| c.segment_type() == Some("select_clause_element"))
                    .count()
                    > 1;
            out.push(Event::Enter(ty));
            for child in children {
                match child {
                    Node::Meta {
                        meta_type: MetaType::Indent { is_implicit },
                        ..
                    } => {
                        out.push(Event::Indent {
                            implicit: *is_implicit,
                            forced: std::mem::take(&mut force),
                        });
                    }
                    _ => walk(child, ty, config, out, newlines),
                }
            }
            out.push(Event::Exit(ty));
        }
        Node::Meta { meta_type, .. } => match meta_type {
            MetaType::Indent { is_implicit } => out.push(Event::Indent {
                implicit: *is_implicit,
                forced: false,
            }),
            MetaType::Dedent { .. } => out.push(Event::Dedent),
            _ => {}
        },
        Node::Unparsable { children, .. } => {
            children
                .iter()
                .for_each(|c| walk(c, parent, config, out, newlines));
        }
        Node::Empty => {}
    }
}
