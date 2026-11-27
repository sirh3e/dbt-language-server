//! A command's JSON answer as a markdown document, for a client that only
//! opens files (Zed): scalars as a list, lists of objects as tables, nested
//! objects as sections, text with line breaks as code blocks. Pure, and
//! bounded: a table shows at most [`ROWS`] rows and [`COLUMNS`] columns,
//! and what is cut says how much.

use serde_json::{Map, Value};

/// Rows a table shows before it says how many more there are.
pub const ROWS: usize = 500;
/// Columns a table shows: the first ones its rows have.
pub const COLUMNS: usize = 8;
/// Characters a cell shows.
const CELL: usize = 80;
/// Heading levels before the rest is shown as JSON.
const DEEPEST: usize = 5;

/// The answer to `title` as a document.
pub fn markdown(title: &str, answer: &Value) -> String {
    let mut out = format!("# {title}\n\n");
    block(&mut out, answer, 2);
    while out.ends_with("\n\n") {
        out.pop();
    }
    out
}

fn block(out: &mut String, value: &Value, level: usize) {
    match value {
        Value::Null => out.push_str("_Nothing._\n\n"),
        Value::Object(map) if map.is_empty() => out.push_str("_Nothing._\n\n"),
        Value::Object(map) => object(out, map, level),
        Value::Array(items) => array(out, items, level),
        Value::String(text) => text_block(out, text),
        other => {
            out.push_str(&scalar(other));
            out.push_str("\n\n");
        }
    }
}

fn is_scalar(value: &Value) -> bool {
    match value {
        Value::String(s) => !s.contains('\n'),
        Value::Array(items) => items.is_empty(),
        Value::Object(map) => map.is_empty(),
        _ => true,
    }
}

fn scalar(value: &Value) -> String {
    match value {
        Value::Null => "–".into(),
        Value::String(s) if s.is_empty() => "\"\"".into(),
        Value::String(s) => s.clone(),
        Value::Array(_) => "none".into(),
        Value::Object(_) => "–".into(),
        other => other.to_string(),
    }
}

fn text_block(out: &mut String, text: &str) {
    if text.contains('\n') {
        let fence = if text.contains("```") { "~~~~" } else { "```" };
        out.push_str(fence);
        out.push('\n');
        out.push_str(text.trim_end_matches('\n'));
        out.push('\n');
        out.push_str(fence);
        out.push_str("\n\n");
    } else {
        out.push_str(text);
        out.push_str("\n\n");
    }
}

fn object(out: &mut String, map: &Map<String, Value>, level: usize) {
    let (plain, nested): (Vec<_>, Vec<_>) = map.iter().partition(|(_, v)| is_scalar(v));
    for (key, value) in &plain {
        out.push_str(&format!("- **{key}**: {}\n", scalar(value)));
    }
    if !plain.is_empty() {
        out.push('\n');
    }
    for (key, value) in nested {
        if level > DEEPEST {
            out.push_str(&format!("**{key}**\n\n"));
            json_block(out, value);
            continue;
        }
        out.push_str(&format!("{} {key}\n\n", "#".repeat(level)));
        block(out, value, level + 1);
    }
}

fn json_block(out: &mut String, value: &Value) {
    let text = serde_json::to_string_pretty(value).unwrap_or_default();
    out.push_str("```json\n");
    out.push_str(&text);
    out.push_str("\n```\n\n");
}

fn array(out: &mut String, items: &[Value], level: usize) {
    if items.iter().all(|v| v.is_object()) {
        return table(out, items, level);
    }
    for value in items.iter().take(ROWS) {
        if is_scalar(value) {
            out.push_str(&format!("- {}\n", scalar(value)));
        } else {
            out.push_str("- ");
            out.push_str(&cell(value));
            out.push('\n');
        }
    }
    more(out, items.len());
    out.push('\n');
}

fn more(out: &mut String, total: usize) {
    if total > ROWS {
        out.push_str(&format!("\n_… and {} more._\n", total - ROWS));
    }
}

/// A list of objects: one row each, the columns their scalar fields.
fn table(out: &mut String, items: &[Value], level: usize) {
    let mut columns: Vec<&str> = Vec::new();
    for item in items {
        for (key, value) in item.as_object().into_iter().flatten() {
            if !columns.contains(&key.as_str()) && (is_scalar(value) || value.is_array()) {
                columns.push(key);
            }
        }
    }
    if columns.is_empty() {
        for (i, item) in items.iter().take(ROWS).enumerate() {
            out.push_str(&format!("{} {}\n\n", "#".repeat(level.min(6)), i + 1));
            block(out, item, level + 1);
        }
        more(out, items.len());
        return;
    }
    let cut = columns.len() > COLUMNS;
    columns.truncate(COLUMNS);
    out.push_str(&format!("| {} |\n", columns.join(" | ")));
    out.push_str(&format!("|{}\n", "---|".repeat(columns.len())));
    for item in items.iter().take(ROWS) {
        let row: Vec<String> = columns
            .iter()
            .map(|c| item.get(*c).map(cell).unwrap_or_default())
            .collect();
        out.push_str(&format!("| {} |\n", row.join(" | ")));
    }
    more(out, items.len());
    if cut {
        out.push_str(&format!(
            "\n_Only the first {COLUMNS} columns are shown._\n"
        ));
    }
    out.push('\n');
}

/// A value in one table cell: one line, `|` escaped, cut at [`CELL`].
fn cell(value: &Value) -> String {
    let text = match value {
        Value::Array(items) if items.iter().all(is_scalar) => {
            items.iter().map(scalar).collect::<Vec<_>>().join(", ")
        }
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other if is_scalar(other) => scalar(other),
        other => other.to_string(),
    };
    let one_line = text.replace('\n', " ").replace('|', "\\|");
    if one_line.chars().count() > CELL {
        let cut: String = one_line.chars().take(CELL - 1).collect();
        format!("{cut}…")
    } else {
        one_line
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn scalars_lists_and_tables() {
        let answer = json!({
            "model": "orders",
            "count": 2,
            "models": [
                {"name": "a", "uri": "file:///a.sql", "reasons": ["no ref", "no exposure"]},
                {"name": "b|c", "uri": "file:///b.sql", "reasons": []}
            ],
            "notes": ["one", "two"]
        });
        assert_eq!(
            markdown("Models nothing uses", &answer),
            "# Models nothing uses\n\n\
             - **count**: 2\n- **model**: orders\n\n\
             ## models\n\n\
             | name | reasons | uri |\n|---|---|---|\n\
             | a | no ref, no exposure | file:///a.sql |\n\
             | b\\|c |  | file:///b.sql |\n\n\
             ## notes\n\n- one\n- two\n"
        );
    }

    #[test]
    fn text_with_line_breaks_is_a_code_block() {
        let answer = json!({ "sexp": "(split\n  orders)" });
        assert_eq!(
            markdown("Plan", &answer),
            "# Plan\n\n## sexp\n\n```\n(split\n  orders)\n```\n"
        );
    }

    #[test]
    fn nothing_says_so() {
        assert_eq!(markdown("X", &Value::Null), "# X\n\n_Nothing._\n");
        assert_eq!(markdown("X", &json!({})), "# X\n\n_Nothing._\n");
    }

    #[test]
    fn a_long_table_says_how_much_was_cut() {
        let rows: Vec<Value> = (0..ROWS + 3).map(|i| json!({ "i": i })).collect();
        let text = markdown("Rows", &json!({ "rows": rows }));
        assert!(text.contains("| 499 |"));
        assert!(!text.contains("| 500 |"));
        assert!(text.ends_with("_… and 3 more._\n"), "{text}");
    }

    #[test]
    fn a_wide_table_keeps_its_first_columns() {
        let row: Map<String, Value> = (0..12).map(|i| (format!("c{i:02}"), json!(i))).collect();
        let text = markdown("Wide", &json!([Value::Object(row)]));
        assert!(text.contains("| c00 | c01 |"));
        assert!(!text.contains("c08"));
        assert!(text.contains("_Only the first 8 columns are shown._"));
    }
}
