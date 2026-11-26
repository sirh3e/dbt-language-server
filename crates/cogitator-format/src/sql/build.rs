//! The layout of a run of events: sqlfluff's Indent points become groups
//! that break when they do not fit, its clauses start lines, and a few
//! segments get a group of their own.

use std::collections::HashMap;

use super::doc::{self, Doc};
use super::events::Event;
use super::sep::{closes, separator, Pending};
use super::space::{self, Leaf};
use crate::config::Config;

/// Segments that start on a line of their own (sqlfluff's clause layout).
const OWN_LINE: [&str; 14] = [
    "from_clause",
    "where_clause",
    "groupby_clause",
    "orderby_clause",
    "having_clause",
    "limit_clause",
    "qualify_clause",
    "named_window",
    "offset_clause",
    "fetch_clause",
    "join_clause",
    "set_operator",
    "select_statement",
    "values_clause",
];

/// Segments that may start a line of the segment they are in.
const MAY_START_A_LINE: [&str; 3] = ["join_on_condition", "when_clause", "else_clause"];

/// Segments with a group of their own, opened at their first token: they
/// break inside only when they do not fit themselves (an expression's
/// operators, a join before its condition, a `case`'s `when`s).
const OWN_GROUP: [&str; 3] = ["join_clause", "expression", "case_expression"];

/// What a Jinja statement does to the lines after it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagRole {
    /// `{% if %}`, `{% for %}`, `{% macro %}`: the lines after it go deeper.
    Open,
    /// `{% else %}`, `{% elif %}`: its own line goes back, the next ones in.
    Mid,
    /// `{% endif %}` and the other `end…`.
    Close,
    /// Every other tag and comment.
    Other,
}

/// The tokens that stand for Jinja in the masked SQL.
#[derive(Debug, Clone, Default)]
pub struct Placeholders {
    /// Every placeholder starts with it (and nothing else in the source does).
    pub prefix: String,
    pub roles: HashMap<String, TagRole>,
}

impl Placeholders {
    pub fn is(&self, raw: &str) -> bool {
        !self.prefix.is_empty() && raw.to_lowercase().contains(&self.prefix)
    }

    fn role(&self, raw: &str) -> TagRole {
        self.roles.get(raw).copied().unwrap_or(TagRole::Other)
    }
}

enum Frame {
    Root(Vec<Doc>),
    /// A group; `closing`: its nest has ended and it waits for the token
    /// that closes it (a bracket, `end`).
    Group {
        docs: Vec<Doc>,
        forced: bool,
        closing: bool,
    },
    Nest(Vec<Doc>),
}

struct Frames(Vec<Frame>);

impl Frames {
    fn top(&mut self) -> &mut Vec<Doc> {
        match self.0.last_mut().expect("the root frame") {
            Frame::Root(docs) | Frame::Nest(docs) | Frame::Group { docs, .. } => docs,
        }
    }

    fn push(&mut self, doc: Doc) {
        self.top().push(doc);
    }

    fn depth(&self) -> usize {
        self.0.len()
    }

    /// A group and its nest.
    fn open(&mut self, forced: bool) {
        self.0.push(Frame::Group {
            docs: Vec::new(),
            forced,
            closing: false,
        });
        self.0.push(Frame::Nest(Vec::new()));
    }

    fn close(&mut self) {
        let doc = match self.0.pop() {
            Some(Frame::Nest(docs)) => Doc::Nest(docs),
            Some(Frame::Group { docs, forced, .. }) => doc::group(docs, forced),
            Some(root @ Frame::Root(_)) => {
                self.0.push(root);
                return;
            }
            None => return,
        };
        self.push(doc);
    }

    fn close_to(&mut self, depth: usize) {
        while self.depth() > depth.max(1) {
            self.close();
        }
    }

    /// A group whose nest ended, when no token follows to close it.
    fn close_waiting(&mut self) {
        while let Some(Frame::Group { closing: true, .. }) = self.0.last() {
            self.close();
        }
    }

    /// A Dedent: the nest ends; its group waits for a closing token.
    /// Whether there was a nest to end.
    fn dedent(&mut self) -> bool {
        self.close_waiting();
        if !matches!(self.0.last(), Some(Frame::Nest(_))) {
            return false;
        }
        self.close();
        if let Some(Frame::Group { closing, .. }) = self.0.last_mut() {
            *closing = true;
        }
        true
    }
}

/// The document of a run of events.
pub fn layout(events: &[Event<'_>], config: &Config, tags: &Placeholders) -> Vec<Doc> {
    let mut frames = Frames(vec![Frame::Root(Vec::new())]);
    let mut pending = Pending::default();
    let mut prev: Option<Leaf<'_>> = None;
    // segments with a group of their own: the frame depth it starts at,
    // None until their first token opens it
    let mut own: Vec<(&str, Option<usize>)> = Vec::new();
    let indented_joins = config.layout.indented_joins;
    for event in events {
        match event {
            Event::Enter(ty) => {
                pending.hard |= OWN_LINE.contains(ty);
                pending.soft |= MAY_START_A_LINE.contains(ty);
                // sqlfluff's tree has a join at `from`'s level already
                pending.shift += isize::from(*ty == "join_clause" && indented_joins);
                if OWN_GROUP.contains(ty) {
                    own.push((ty, None));
                }
            }
            Event::Exit(ty) => {
                if OWN_GROUP.contains(ty) {
                    if let Some((_, Some(depth))) = own.pop() {
                        frames.close_to(depth);
                    }
                }
                if *ty == "join_clause" && indented_joins {
                    frames.push(Doc::Shift(-1));
                }
                pending.after_cte |= *ty == "common_table_expression";
            }
            Event::Indent { implicit, forced } => {
                frames.close_waiting();
                frames.open(*forced);
                // the table after `join` stays on its line, as an implicit
                // indent's first line does
                let after_join = prev
                    .as_ref()
                    .is_some_and(|p| p.is_keyword && p.raw.eq_ignore_ascii_case("join"));
                pending.indent.get_or_insert(*implicit || after_join);
            }
            Event::Dedent => pending.dedent |= frames.dedent(),
            Event::Leaf(leaf) => {
                let role = if tags.is(leaf.raw) {
                    tags.role(leaf.raw)
                } else {
                    TagRole::Other
                };
                if !(pending.dedent && closes(leaf)) {
                    frames.close_waiting();
                }
                if let Some(prev) = &prev {
                    let blocks = config.jinja.indent_blocks;
                    let shift = pending.shift
                        - isize::from(blocks && matches!(role, TagRole::Close | TagRole::Mid));
                    if shift != 0 {
                        frames.push(Doc::Shift(shift));
                    }
                    // a tag written inline stays inline, so that whether a
                    // tag stands alone is its author's choice, every run
                    let inline_tag = leaf.newlines_before == 0
                        && (tags.is(prev.raw) || tags.is(leaf.raw))
                        && prev.ty != "inline_comment";
                    let sep = separator(prev, leaf, &pending, config, inline_tag);
                    frames.push(Doc::Sep(sep));
                }
                for (_, depth) in own.iter_mut().filter(|(_, d)| d.is_none()) {
                    *depth = Some(frames.depth());
                    frames.open(false);
                }
                frames.push(Doc::Text(space::cased(leaf, &config.case, |raw| {
                    tags.is(raw)
                })));
                if closes(leaf) {
                    frames.close_waiting();
                }
                if config.jinja.indent_blocks && matches!(role, TagRole::Open | TagRole::Mid) {
                    frames.push(Doc::Shift(1));
                }
                pending = Pending {
                    after_cte: pending.after_cte && leaf.ty == "comma",
                    ..Pending::default()
                };
                prev = Some(leaf.clone());
            }
        }
    }
    frames.close_to(1);
    match frames.0.pop() {
        Some(Frame::Root(docs)) => docs,
        _ => Vec::new(),
    }
}
