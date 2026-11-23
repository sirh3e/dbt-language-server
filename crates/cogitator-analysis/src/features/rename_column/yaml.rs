//! The properties-YAML side of a renamed column of model `M`:
//!
//! - M's `columns:` entry name (its column tests move with it);
//! - inside M's entry, the lists that name columns: `constraints:` →
//!   `columns:`, `primary_key:`, `combination_of_columns:`, and any
//!   `column_name:` argument;
//! - anywhere, a `relationships` test's `field:` whose sibling `to:` is
//!   `ref('M')`;
//! - unit tests: `expect` row keys of a test of M, and `given` row keys of
//!   an `input: ref('M')`. A csv or sql fixture naming the column is a
//!   refusal: its text is not keyed rows.
//!
//! `description:` text and `docs` blocks are left alone.

use std::path::Path;

use cogitator_lsp::Range;

use super::super::references::occur::name_span;
use super::super::DbtContext;
use super::yaml_lines::{block_end, entry_indent, flow_items, key_line, row_keys, scalar_at};

const COLUMN_LISTS: [&str; 3] = ["columns", "primary_key", "combination_of_columns"];

/// M's own entry: the column's name and the lists of M's block.
pub(super) fn entry_sites(
    ctx: &DbtContext,
    text: &str,
    model: &str,
    column: &str,
) -> (Option<Range>, Vec<Range>) {
    let Some(details) = ctx.model_detail_map.get(model) else {
        return (None, Vec::new());
    };
    let name = details
        .columns
        .iter()
        .find(|c| c.name.eq_ignore_ascii_case(column))
        .and_then(|c| name_span(text, c.name_pos, &c.name));
    let lines: Vec<&str> = text.lines().collect();
    let start = details.schema_range.start.line as usize;
    let mut lists = Vec::new();
    let Some(first) = lines.get(start) else {
        return (name, lists);
    };
    let end = block_end(&lines, start, entry_indent(first));
    let mut open_list: Option<usize> = None;
    for (no, line) in lines.iter().enumerate().take(end + 1).skip(start) {
        let trimmed = line.trim_start();
        if let (Some(list), Some(item)) = (open_list, trimmed.strip_prefix("- ")) {
            let at = line.len() - item.len();
            if entry_indent(line) > list && !item.contains(':') {
                lists.extend(scalar_at(no, item.trim_end(), at, column));
                continue;
            }
        }
        open_list = None;
        let Some(k) = key_line(line) else {
            continue;
        };
        if k.key == "column_name" {
            lists.extend(scalar_at(no, k.value, k.value_at, column));
        } else if COLUMN_LISTS.contains(&k.key) {
            if k.value.is_empty() {
                open_list = Some(k.at);
            } else {
                lists.extend(flow_items(no, k.value, k.value_at, column));
                lists.extend(scalar_at(no, k.value, k.value_at, column));
            }
        }
    }
    (name, lists)
}

/// Whether a `to:`/`input:` value is `ref('model')`.
fn refs(value: &str, model: &str) -> bool {
    let v: String = value.chars().filter(|c| !c.is_whitespace()).collect();
    v == format!("ref('{model}')") || v == format!("ref(\"{model}\")")
}

/// `field: <column>` of a relationships test pointing at `model`.
pub(super) fn relationship_sites(text: &str, model: &str, column: &str) -> Vec<Range> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    for (no, line) in lines.iter().enumerate() {
        let Some(k) = key_line(line).filter(|k| k.key == "field") else {
            continue;
        };
        let Some(range) = scalar_at(no, k.value, k.value_at, column) else {
            continue;
        };
        let deep = |l: &&&str| super::yaml_lines::indent(l) >= k.at || l.trim().is_empty();
        let near = lines[..no]
            .iter()
            .rev()
            .take_while(deep)
            .chain(lines[no + 1..].iter().take_while(deep))
            .filter_map(|l| key_line(l))
            .any(|s| s.at == k.at && s.key == "to" && refs(s.value, model));
        if near {
            out.push(range);
        }
    }
    out
}

/// Row keys of unit tests: `expect` of a test of a renamed model, `given`
/// of an input that refs one. `renamed(model)` says which models are.
pub(super) fn unit_test_sites(
    text: &str,
    column: &str,
    renamed: &dyn Fn(&str) -> bool,
    path: &Path,
) -> Result<Vec<Range>, String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    let Some(top) = lines.iter().position(|l| l.trim_end() == "unit_tests:") else {
        return Ok(out);
    };
    let end = block_end(&lines, top, 0);
    let mut test_model: Option<String> = None;
    let mut keyed = false;
    let mut rows_indent: Option<usize> = None;
    let mut fixture_format = false;
    for (no, line) in lines.iter().enumerate().take(end + 1).skip(top + 1) {
        let line = *line;
        let k = key_line(line);
        if let Some(ri) = rows_indent {
            if super::yaml_lines::indent(line) > ri || line.trim().is_empty() {
                if keyed {
                    out.extend(row_keys(no, line, column));
                }
                if fixture_format && keyed && mentions(line, column) {
                    return Err(format!(
                        "{}: a csv or sql unit-test fixture names `{column}`; edit it by hand",
                        path.display()
                    ));
                }
                continue;
            }
            rows_indent = None;
        }
        let Some(k) = k else { continue };
        let dash = line.trim_start().starts_with("- ");
        match k.key {
            "model" => test_model = Some(k.value.trim_matches(['\'', '"']).to_string()),
            "name" if dash && super::yaml_lines::indent(line) <= 2 => {
                test_model = None;
                keyed = false;
            }
            "input" => {
                keyed = renamed_ref(k.value).is_some_and(|m| renamed(&m));
                fixture_format = false;
            }
            "expect" => {
                keyed = test_model.as_deref().is_some_and(renamed);
                fixture_format = false;
            }
            "format" => fixture_format = matches!(k.value, "csv" | "sql"),
            "rows" => rows_indent = Some(k.at),
            _ => {}
        }
    }
    Ok(out)
}

fn renamed_ref(value: &str) -> Option<String> {
    let v: String = value.chars().filter(|c| !c.is_whitespace()).collect();
    let inner = v.strip_prefix("ref(")?.strip_suffix(')')?;
    Some(inner.trim_matches(['\'', '"']).to_string())
}

fn mentions(line: &str, column: &str) -> bool {
    line.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .any(|w| w.eq_ignore_ascii_case(column))
}
