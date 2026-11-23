//! Line-level reading of properties YAML for the column rename: keys,
//! scalar values and the extent of a block, as ranges into the text as
//! written (byte columns, as every other range of the server). The YAML
//! is never re-serialized.

use cogitator_lsp::Range;

use super::super::references::occur::span;

pub(super) fn indent(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

fn is_content(line: &str) -> bool {
    let t = line.trim();
    !t.is_empty() && !t.starts_with('#')
}

/// One `key: value` line, with a leading `- ` taken off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct KeyLine<'a> {
    /// Column of the key.
    pub at: usize,
    pub key: &'a str,
    /// The value as written (comment and outer spaces off), and its column.
    pub value: &'a str,
    pub value_at: usize,
}

pub(super) fn key_line(line: &str) -> Option<KeyLine<'_>> {
    let mut at = indent(line);
    let mut rest = &line[at..];
    if let Some(after) = rest.strip_prefix("- ") {
        at += 2 + indent(after);
        rest = after.trim_start();
    }
    let colon = rest.find(':')?;
    let key = rest[..colon].trim();
    if key.is_empty() || key.contains([' ', '{', '[', '\'', '"']) {
        return None;
    }
    let after = &rest[colon + 1..];
    if !(after.is_empty() || after.starts_with(' ')) {
        return None;
    }
    let raw = after.split(" #").next().unwrap_or("");
    let value = raw.trim();
    let value_at = at + colon + 1 + (raw.len() - raw.trim_start().len());
    Some(KeyLine {
        at,
        key,
        value,
        value_at,
    })
}

/// The last line of the block opened at `start`: every later content line
/// indented deeper than `parent`.
pub(super) fn block_end(lines: &[&str], start: usize, parent: usize) -> usize {
    let mut last = start;
    for (i, line) in lines.iter().enumerate().skip(start + 1) {
        if !is_content(line) {
            continue;
        }
        if indent(line) <= parent {
            break;
        }
        last = i;
    }
    last
}

/// The column of an entry's dash (`- name: x` → the `-`), or its key.
pub(super) fn entry_indent(line: &str) -> usize {
    indent(line)
}

/// A scalar equal to `name` (quotes allowed, case ignored), as the range
/// of the name inside its quotes.
pub(super) fn scalar_at(line_no: usize, value: &str, value_at: usize, name: &str) -> Option<Range> {
    let quoted = value.starts_with(['\'', '"']);
    let inner = if quoted {
        value.get(1..value.len().checked_sub(1)?)?
    } else {
        value
    };
    inner.eq_ignore_ascii_case(name).then(|| {
        span(
            line_no as u32,
            (value_at + quoted as usize) as u32,
            name.len() as u32,
        )
    })
}

/// The items of a flow list `[a, 'b', c]` equal to `name`.
pub(super) fn flow_items(line_no: usize, value: &str, value_at: usize, name: &str) -> Vec<Range> {
    let Some(inner) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut offset = value_at + 1;
    for item in inner.split(',') {
        let lead = item.len() - item.trim_start().len();
        if let Some(r) = scalar_at(line_no, item.trim(), offset + lead, name) {
            out.push(r);
        }
        offset += item.len() + 1;
    }
    out
}

/// Keys equal to `name` in a row line: `- old: 1`, `old: 1` or a flow
/// mapping `{old: 1, b: 2}` (quoted keys too).
pub(super) fn row_keys(line_no: usize, line: &str, name: &str) -> Vec<Range> {
    let mut out = Vec::new();
    let body = line.trim_start().trim_start_matches("- ").trim_start();
    let start = line.len() - body.len();
    if let Some(inner) = body.strip_prefix('{') {
        let inner = inner.split('}').next().unwrap_or("");
        let mut offset = start + 1;
        for part in inner.split(',') {
            if let Some(colon) = part.find(':') {
                let key = &part[..colon];
                let lead = key.len() - key.trim_start().len();
                if let Some(r) = scalar_at(line_no, key.trim(), offset + lead, name) {
                    out.push(r);
                }
            }
            offset += part.len() + 1;
        }
    } else if let Some(k) = key_line(line) {
        let key_start = line[k.at..].find(k.key).map_or(k.at, |i| k.at + i);
        if let Some(r) = scalar_at(line_no, k.key, key_start, name) {
            out.push(r);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cols(r: &[Range]) -> Vec<(u32, u32)> {
        r.iter()
            .map(|r| (r.start.character, r.end.character))
            .collect()
    }

    #[test]
    fn key_lines() {
        for (line, expected) in [
            ("  - name: orders", Some(("name", "orders", 10))),
            (
                "    field: customer_id  # fk",
                Some(("field", "customer_id", 11)),
            ),
            ("to: ref('customers')", Some(("to", "ref('customers')", 4))),
            ("  rows:", Some(("rows", "", 7))),
            ("  - {a: 1}", None),
            ("  url: http://x", Some(("url", "http://x", 7))),
        ] {
            let got = key_line(line).map(|k| (k.key, k.value, k.value_at));
            assert_eq!(got, expected, "{line:?}");
        }
    }

    #[test]
    fn scalars_lists_and_rows() {
        assert_eq!(
            cols(&flow_items(0, "[a, 'old', OLD]", 5, "old")),
            vec![(10, 13), (16, 19)]
        );
        for (line, expected) in [
            ("      - {old: 1, b: 2}", vec![(9, 12)]),
            ("      - {a: 1, \"old\": 2}", vec![(16, 19)]),
            ("      - old: 1", vec![(8, 11)]),
            ("        old: 1", vec![(8, 11)]),
            ("        older: 1", vec![]),
        ] {
            assert_eq!(cols(&row_keys(0, line, "old")), expected, "{line:?}");
        }
        assert!(scalar_at(0, "'old'", 4, "old").is_some());
        assert!(scalar_at(0, "olds", 4, "old").is_none());
    }
}
