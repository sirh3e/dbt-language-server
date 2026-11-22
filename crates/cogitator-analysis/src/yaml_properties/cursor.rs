//! Where the cursor stands in a properties YAML file: the path of keys
//! and list items from the document root down to the mapping (or list)
//! being typed in, and what is being typed there. Line-wise over the
//! text, so a half-typed file (which no YAML parser loads) still
//! answers: the path follows indentation and `- ` items upward.

use std::sync::LazyLock;

use cogitator_lsp::Position;

/// One step from the root: a mapping key, or an item of a list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathStep {
    Key(String),
    Item,
}

/// What the cursor is typing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Slot {
    /// A key of the mapping at the path.
    Key { partial: String },
    /// The value of `key` in the mapping at the path.
    Value { key: String, partial: String },
    /// A `- …` item of the list at the path (its last step is the key
    /// that holds the list).
    Item { partial: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YamlCursor {
    pub path: Vec<PathStep>,
    pub slot: Slot,
}

/// One line as the path walk sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Line<'a> {
    /// Column of the first non-space character.
    indent: usize,
    /// Column of a leading `-` of a list item.
    dash: Option<usize>,
    /// A `key:` on the line: its name, column and whether a value
    /// follows on the same line.
    key: Option<(&'a str, usize, bool)>,
}

/// `key:` at the start of `rest` (plain or quoted), then what follows.
static KEY: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"^("[^"]*"|'[^']*'|[^\s:#'"\-][^:#]*?|-[^\s:#][^:#]*?)\s*:(?:\s+|$)(.*)$"#)
        .expect("static regex")
});

fn parse(line: &str) -> Option<Line<'_>> {
    let trimmed = line.trim_start_matches(' ');
    if trimmed.trim().is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let indent = line.len() - trimmed.len();
    let (dash, body_col) = match dash_body(trimmed) {
        Some(offset) => (Some(indent), indent + offset),
        None => (None, indent),
    };
    let key = KEY.captures(&line[body_col..]).map(|c| {
        let name = c
            .get(1)
            .map_or("", |m| m.as_str())
            .trim_matches(['"', '\'']);
        let value = c.get(2).map_or("", |m| m.as_str()).trim();
        (name, body_col, !value.is_empty() && !value.starts_with('#'))
    });
    Some(Line { indent, dash, key })
}

/// After `- `: the offset of the item's body (its first key or scalar).
fn dash_body(trimmed: &str) -> Option<usize> {
    let rest = trimmed.strip_prefix('-')?;
    if !(rest.is_empty() || rest.starts_with(' ')) {
        return None;
    }
    Some(1 + rest.len() - rest.trim_start_matches(' ').len())
}

/// The walk's state: inside a mapping whose keys sit at `col` (an item
/// of a list when its first key shares a line with the dash at
/// `item_dash`), or inside a list whose dashes sit at `dash`.
#[derive(Debug, Clone, Copy)]
enum State {
    Mapping {
        col: usize,
        item_dash: Option<usize>,
    },
    List {
        dash: usize,
    },
}

/// The cursor's path and slot; None past the text's end.
pub fn cursor_at(text: &str, pos: Position) -> Option<YamlCursor> {
    let lines: Vec<&str> = text.split('\n').map(|l| l.trim_end_matches('\r')).collect();
    let row = pos.line as usize;
    let line = *lines.get(row)?;
    let prefix = utf16_prefix(line, pos.character as usize);
    let trimmed = prefix.trim_start_matches(' ');
    let indent = prefix.len() - trimmed.len();
    let (dash, body_col) = match dash_body(trimmed) {
        Some(offset) => (Some(indent), indent + offset),
        None => (None, indent),
    };
    let body = &prefix[body_col..];
    let (slot, state) = if let Some(c) = KEY.captures(body) {
        let key = c
            .get(1)
            .map_or("", |m| m.as_str())
            .trim_matches(['"', '\'']);
        let partial = c.get(2).map_or("", |m| m.as_str()).trim_start();
        (
            Slot::Value {
                key: key.to_string(),
                partial: partial.to_string(),
            },
            State::Mapping {
                col: body_col,
                item_dash: dash,
            },
        )
    } else if let Some(d) = dash {
        (
            Slot::Item {
                partial: body.to_string(),
            },
            State::List { dash: d },
        )
    } else {
        (
            Slot::Key {
                partial: body.to_string(),
            },
            State::Mapping {
                col: indent,
                item_dash: None,
            },
        )
    };
    let parsed: Vec<Option<Line<'_>>> = lines[..row].iter().map(|l| parse(l)).collect();
    let path = walk_up(&parsed, state);
    Some(YamlCursor { path, slot })
}

/// From the cursor's state upward to the root, collecting the steps.
fn walk_up(lines: &[Option<Line<'_>>], mut state: State) -> Vec<PathStep> {
    let mut steps = Vec::new();
    let mut cur = lines.len();
    loop {
        match state {
            State::Mapping {
                item_dash: Some(d), ..
            } => {
                steps.push(PathStep::Item);
                state = State::List { dash: d };
            }
            State::Mapping { col: 0, .. } => break,
            State::Mapping {
                col,
                item_dash: None,
            } => {
                let Some((at, line)) = above(lines, cur, |l| l.indent < col) else {
                    break;
                };
                cur = at;
                match (line.key, line.dash) {
                    // the dash line that opens this mapping's item
                    (Some((_, key_col, _)), Some(d)) if key_col == col => {
                        state = State::Mapping {
                            col,
                            item_dash: Some(d),
                        };
                    }
                    (Some((name, key_col, false)), dash) if key_col < col => {
                        steps.push(PathStep::Key(name.to_string()));
                        state = opened_by(key_col, dash);
                    }
                    _ => break,
                }
            }
            State::List { dash } => {
                let found = above(lines, cur, |l| match l.key {
                    Some((_, key_col, _)) => key_col <= dash && l.dash != Some(dash),
                    None => l.indent < dash,
                });
                let Some((at, line)) = found else { break };
                cur = at;
                match line.key {
                    Some((name, key_col, false)) => {
                        steps.push(PathStep::Key(name.to_string()));
                        state = opened_by(key_col, line.dash);
                    }
                    _ => break,
                }
            }
        }
    }
    steps.reverse();
    steps
}

/// The mapping a key at `key_col` belongs to: an item's, when the key
/// shares its line with a dash.
fn opened_by(key_col: usize, dash: Option<usize>) -> State {
    State::Mapping {
        col: key_col,
        item_dash: dash,
    }
}

/// The nearest content line above `cur` that `stop` accepts.
fn above<'a>(
    lines: &[Option<Line<'a>>],
    cur: usize,
    stop: impl Fn(&Line<'a>) -> bool,
) -> Option<(usize, Line<'a>)> {
    (0..cur)
        .rev()
        .filter_map(|i| lines[i].map(|l| (i, l)))
        .find(|(_, l)| stop(l))
}

/// The bytes of `line` before UTF-16 column `character`.
fn utf16_prefix(line: &str, character: usize) -> &str {
    let mut units = 0;
    for (i, c) in line.char_indices() {
        if units >= character {
            return &line[..i];
        }
        units += c.len_utf16();
    }
    line
}

#[cfg(test)]
#[path = "cursor_tests.rs"]
mod tests;
