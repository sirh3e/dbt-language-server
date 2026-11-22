//! Line-level reading of YAML text for the actions that insert into it:
//! indents, the extent of a block, and an insertion point that stays a
//! valid position when the file does not end in a newline. Pure over the
//! text — the YAML stays as written, comments and all.

use cogitator_lsp::{Position, Range, TextEdit};

/// Spaces before the first non-space character.
pub(super) fn indent(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

/// A line that carries YAML content (not blank, not only a comment).
pub(super) fn is_content(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty() && !trimmed.starts_with('#')
}

/// The last content line of the block that starts at `start` and holds
/// every following line indented deeper than `parent_indent`.
pub(super) fn block_end(lines: &[&str], start: usize, parent_indent: usize) -> usize {
    let mut last = start;
    for (i, line) in lines.iter().enumerate().skip(start + 1) {
        if !is_content(line) {
            continue;
        }
        if indent(line) <= parent_indent {
            break;
        }
        last = i;
    }
    last
}

/// A TextEdit inserting `body` (complete lines, each ending in `\n`) on
/// the line after `after`. At the end of a file without a final newline
/// the insertion goes at the end of the last line, behind a newline.
pub(super) fn insert_after_line(text: &str, after: usize, body: String) -> TextEdit {
    let lines: Vec<&str> = text.lines().collect();
    let at_end = after + 1 >= lines.len();
    if at_end && !text.is_empty() && !text.ends_with('\n') {
        let last = lines.len().saturating_sub(1);
        let character = lines.get(last).map_or(0, |l| l.encode_utf16().count()) as u32;
        let end = Position {
            line: last as u32,
            character,
        };
        return TextEdit {
            range: Range::zero_width(end),
            new_text: format!("\n{body}"),
        };
    }
    TextEdit {
        range: Range::zero_width(Position {
            line: (after + 1) as u32,
            character: 0,
        }),
        new_text: body,
    }
}

/// One `- …` item of a YAML list: its dash line, the indent of its keys
/// and its last content line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Entry {
    pub dash_line: usize,
    pub key_indent: usize,
    pub end: usize,
}

/// The list item holding `line` (its `name:` line, say): the nearest
/// dash line at or above it.
pub(super) fn entry_at(lines: &[&str], line: usize) -> Option<Entry> {
    let dash_line = (0..=line).rev().find(|&i| {
        lines
            .get(i)
            .is_some_and(|l| l.trim_start().starts_with('-'))
    })?;
    let dash = indent(lines[dash_line]);
    Some(Entry {
        dash_line,
        key_indent: dash + 2,
        end: block_end(lines, dash_line, dash),
    })
}

/// The line of `key:` among the entry's own keys (not nested ones).
pub(super) fn key_line(lines: &[&str], entry: Entry, key: &str) -> Option<usize> {
    let prefix = format!("{key}:");
    let own = |i: usize| {
        let line = lines[i];
        // the dash line carries its first key after `- `
        let at = if i == entry.dash_line {
            indent(line) + 2
        } else {
            indent(line)
        };
        at == entry.key_indent && line[at.min(line.len())..].starts_with(&prefix)
    };
    (entry.dash_line..=entry.end).find(|&i| own(i))
}

/// Append list items under `key:` of the entry holding `line`, or start
/// the list at the entry's end. Each item is its lines without indent,
/// the first beginning `- `. None where `key:` has an inline value.
pub(super) fn append_to_list(
    text: &str,
    line: usize,
    key: &str,
    items: &[Vec<String>],
) -> Option<TextEdit> {
    let lines: Vec<&str> = text.lines().collect();
    let entry = entry_at(&lines, line)?;
    let (after, item_indent, header) = match key_line(&lines, entry, key) {
        Some(at) => {
            let value = lines[at].trim_start().trim_start_matches("- ");
            let value = value[key.len() + 1..].trim();
            if !value.is_empty() && !value.starts_with('#') {
                return None; // `key: []` and the like: left to the reader
            }
            let end = block_end(&lines, at, entry.key_indent);
            let item_indent = (at + 1..=end)
                .find(|&i| is_content(lines[i]) && lines[i].trim_start().starts_with('-'))
                .map_or(entry.key_indent + 2, |i| indent(lines[i]));
            (end, item_indent, None)
        }
        None => (entry.end, entry.key_indent + 2, Some(entry.key_indent)),
    };
    let mut body = String::new();
    if let Some(header_indent) = header {
        body.push_str(&format!("{}{key}:\n", " ".repeat(header_indent)));
    }
    let pad = " ".repeat(item_indent);
    for item in items {
        for piece in item {
            body.push_str(&format!("{pad}{piece}\n"));
        }
    }
    Some(insert_after_line(text, after, body))
}

/// Whole lines `first..=last`, as a range that deletes them.
pub(super) fn lines_range(text: &str, first: usize, last: usize) -> Range {
    let count = text.lines().count();
    let end = if last + 1 < count || text.ends_with('\n') {
        Position {
            line: (last + 1) as u32,
            character: 0,
        }
    } else {
        let line = text.lines().nth(last).unwrap_or("");
        Position {
            line: last as u32,
            character: line.encode_utf16().count() as u32,
        }
    };
    Range {
        start: Position {
            line: first as u32,
            character: 0,
        },
        end,
    }
}

/// A plain YAML scalar for a name, or None when it would need quoting:
/// the actions only write names a reader would write unquoted.
pub(super) fn plain_name(name: &str) -> Option<&str> {
    let ok = !name.is_empty()
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !name.starts_with(|c: char| c.is_ascii_digit());
    ok.then_some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_end_skips_blank_and_comment_lines_inside() {
        let text = "a:\n  b: 1\n\n  # note\n  c: 2\nd: 3\n";
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(block_end(&lines, 0, 0), 4);
    }

    #[test]
    fn insertion_after_the_last_line_with_and_without_a_final_newline() {
        let edit = insert_after_line("a: 1\n", 0, "b: 2\n".into());
        assert_eq!(
            edit.range.start,
            Position {
                line: 1,
                character: 0
            }
        );
        assert_eq!(edit.new_text, "b: 2\n");
        let edit = insert_after_line("a: 1", 0, "b: 2\n".into());
        assert_eq!(
            edit.range.start,
            Position {
                line: 0,
                character: 4
            }
        );
        assert_eq!(edit.new_text, "\nb: 2\n");
    }

    #[test]
    fn the_own_keys_of_an_entry_are_found_not_nested_ones() {
        let text = "- name: a\n  config:\n    group: x\n  group: y\n- name: b\n";
        let lines: Vec<&str> = text.lines().collect();
        let entry = entry_at(&lines, 0).unwrap();
        assert_eq!(entry.end, 3);
        assert_eq!(key_line(&lines, entry, "group"), Some(3));
        assert_eq!(key_line(&lines, entry, "name"), Some(0));
        assert_eq!(key_line(&lines, entry, "access"), None);
    }

    #[test]
    fn lines_range_takes_the_line_break_or_runs_to_the_end() {
        let r = lines_range("a\nb\nc\n", 1, 1);
        assert_eq!((r.start.line, r.end.line, r.end.character), (1, 2, 0));
        let r = lines_range("a\nb", 1, 1);
        assert_eq!((r.start.line, r.end.line, r.end.character), (1, 1, 1));
    }

    #[test]
    fn only_plain_names_are_written() {
        assert_eq!(plain_name("order_id"), Some("order_id"));
        assert_eq!(plain_name("has space"), None);
        assert_eq!(plain_name("1st"), None);
        assert_eq!(plain_name(""), None);
    }
}
