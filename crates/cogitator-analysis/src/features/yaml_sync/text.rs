//! Text plumbing of the sync: applying a phase's edits to the YAML text,
//! turning the before/after of a file into minimal line edits and a
//! unified diff, and the scalars the sync writes. Nothing here parses or
//! re-serializes YAML: what the sync did not touch stays byte for byte.

use cogitator_lsp::{Position, Range, TextEdit};
use similar::{DiffOp, TextDiff};

use super::super::code_actions::yaml_text::{indent, insert_after_line, is_content};

/// The byte offset of an LSP position (UTF-16 characters) in `text`;
/// past a line's end clamps to it, past the last line to the end.
fn offset(text: &str, pos: Position) -> usize {
    let mut line_start = 0;
    for _ in 0..pos.line {
        match text[line_start..].find('\n') {
            Some(i) => line_start += i + 1,
            None => return text.len(),
        }
    }
    let line_end = text[line_start..]
        .find('\n')
        .map_or(text.len(), |i| line_start + i);
    let mut units = 0u32;
    for (i, c) in text[line_start..line_end].char_indices() {
        if units >= pos.character {
            return line_start + i;
        }
        units += c.len_utf16() as u32;
    }
    line_end
}

/// `text` with `edits` applied. The edits must not overlap; their order
/// does not matter (they are spliced from the end).
pub(super) fn apply(text: &str, edits: &[TextEdit]) -> String {
    let mut spans: Vec<(usize, usize, &str)> = edits
        .iter()
        .map(|e| {
            (
                offset(text, e.range.start),
                offset(text, e.range.end),
                e.new_text.as_str(),
            )
        })
        .collect();
    spans.sort_by_key(|s| std::cmp::Reverse((s.0, s.1)));
    let mut out = text.to_string(); // boundary: the phase's new text
    for (start, end, new_text) in spans {
        out.replace_range(start..end, new_text);
    }
    out
}

/// The line edits that turn `old` into `new`, one per changed hunk.
pub(super) fn line_edits(old: &str, new: &str) -> Vec<TextEdit> {
    let diff = TextDiff::from_lines(old, new);
    let (olds, news) = (diff.old_slices(), diff.new_slices());
    let mut out = Vec::new();
    let mut hunk: Option<(usize, usize, usize, usize)> = None;
    let mut flush = |hunk: &mut Option<(usize, usize, usize, usize)>| {
        if let Some((o_start, o_end, n_start, n_end)) = hunk.take() {
            out.push(TextEdit {
                range: Range {
                    start: line_start(old, olds, o_start),
                    end: line_start(old, olds, o_end),
                },
                new_text: news[n_start..n_end].concat(),
            });
        }
    };
    for op in diff.ops() {
        if let DiffOp::Equal { .. } = op {
            flush(&mut hunk);
            continue;
        }
        let (o, n) = (op.old_range(), op.new_range());
        hunk = Some(match hunk {
            Some((os, _, ns, _)) => (os, o.end, ns, n.end),
            None => (o.start, o.end, n.start, n.end),
        });
    }
    flush(&mut hunk);
    out
}

/// Where old line `index` starts; past the last line, the end of the
/// text (which is a line start only when the text ends in a newline).
fn line_start(text: &str, lines: &[&str], index: usize) -> Position {
    if index < lines.len() || text.ends_with('\n') || lines.is_empty() {
        return Position {
            line: index as u32,
            character: 0,
        };
    }
    let last = lines[lines.len() - 1];
    Position {
        line: (lines.len() - 1) as u32,
        character: last.encode_utf16().count() as u32,
    }
}

/// The unified diff a preview shows, three lines of context.
pub(super) fn unified(old: &str, new: &str, name: &str) -> String {
    TextDiff::from_lines(old, new)
        .unified_diff()
        .context_radius(3)
        .header(&format!("a/{name}"), &format!("b/{name}"))
        .to_string()
}

/// A double-quoted YAML scalar holding `value` exactly.
pub(super) fn quoted(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A name as YAML reads it back unchanged: plain when it can be.
pub(super) fn name_scalar(name: &str) -> String {
    let plain = !name.is_empty()
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !name.starts_with(|c: char| c.is_ascii_digit())
        && !matches!(
            name.to_ascii_lowercase().as_str(),
            "true" | "false" | "yes" | "no" | "on" | "off" | "null" | "y" | "n"
        );
    if plain {
        name.to_string()
    } else {
        quoted(name)
    }
}

/// A `data_type:` as written by the type features, quoted only when YAML
/// would read it otherwise.
pub(super) fn type_scalar(value: &str) -> String {
    let plain = !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || " _(),".contains(c));
    if plain {
        value.to_string()
    } else {
        quoted(value)
    }
}

/// Insert `items` (unindented lines, each item's first line `- …`) into
/// the top-level `key:` list of a properties file, or start that list at
/// the end of the file. `None` where `key:` carries an inline value.
pub(super) fn append_top_level(text: &str, key: &str, items: &[Vec<String>]) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    let header = lines.iter().position(|l| {
        indent(l) == 0
            && l.strip_prefix(key)
                .and_then(|rest| rest.strip_prefix(':'))
                .is_some()
    });
    let mut body = String::new();
    let (after, pad) = match header {
        Some(at) => {
            let value = lines[at][key.len() + 1..].trim();
            if !value.is_empty() && !value.starts_with('#') {
                return None; // `models: []` and the like
            }
            // the list's items may sit at indent 0 under a top-level key
            let (mut end, mut pad) = (at, None);
            for (i, line) in lines.iter().enumerate().skip(at + 1) {
                if !is_content(line) {
                    continue;
                }
                let dash = line.trim_start().starts_with('-');
                if indent(line) == 0 && !dash {
                    break;
                }
                if dash && pad.is_none() {
                    pad = Some(indent(line));
                }
                end = i;
            }
            (end, pad.unwrap_or(2))
        }
        None => {
            if !text.is_empty() && !text.ends_with("\n\n") {
                body.push('\n');
            }
            body.push_str(&format!("{key}:\n"));
            (lines.len().saturating_sub(1), 2)
        }
    };
    let pad = " ".repeat(pad);
    for item in items {
        for piece in item {
            body.push_str(&format!("{pad}{piece}\n"));
        }
    }
    Some(apply(text, &[insert_after_line(text, after, body)]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_edits_reproduce_the_new_text() {
        let cases = [
            ("a\nb\nc\n", "a\nx\nb\nc\n"),
            ("a\nb\nc\n", "a\nc\n"),
            ("a\nb", "a\nb\nc\n"),
            ("a\nb\n", "z\nb\ny\n"),
            ("", "version: 2\n"),
        ];
        for (old, new) in cases {
            let edits = line_edits(old, new);
            assert_eq!(apply(old, &edits), new, "{old:?} -> {new:?}");
        }
    }

    #[test]
    fn offsets_count_utf16_units() {
        let text = "é😀x\nnext";
        let pos = Position {
            line: 0,
            character: 3,
        };
        assert_eq!(&text[offset(text, pos)..], "x\nnext");
    }

    #[test]
    fn scalars_round_trip() {
        assert_eq!(quoted("say \"hi\"\nnow"), "\"say \\\"hi\\\"\\nnow\"");
        assert_eq!(name_scalar("order_id"), "order_id");
        assert_eq!(name_scalar("Order Id"), "\"Order Id\"");
        assert_eq!(name_scalar("yes"), "\"yes\"");
        assert_eq!(type_scalar("decimal(18, 2)"), "decimal(18, 2)");
        assert_eq!(type_scalar("struct<a int>"), "\"struct<a int>\"");
    }

    #[test]
    fn top_level_lists_take_items_at_their_own_indent() {
        let item = vec!["- name: b".to_string()];
        let text = "version: 2\nmodels:\n  - name: a\n    description: x\nsources: []\n";
        assert_eq!(
            append_top_level(text, "models", std::slice::from_ref(&item)).unwrap(),
            "version: 2\nmodels:\n  - name: a\n    description: x\n  - name: b\nsources: []\n"
        );
        let flat = "models:\n- name: a\n  columns:\n  - name: id\n";
        assert_eq!(
            append_top_level(flat, "models", std::slice::from_ref(&item)).unwrap(),
            "models:\n- name: a\n  columns:\n  - name: id\n- name: b\n"
        );
        assert_eq!(
            append_top_level("version: 2\n", "models", std::slice::from_ref(&item)).unwrap(),
            "version: 2\n\nmodels:\n  - name: b\n"
        );
        assert!(append_top_level("models: []\n", "models", &[item]).is_none());
    }
}
