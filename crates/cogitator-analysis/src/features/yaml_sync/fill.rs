//! The phases that edit written columns: prune the stale ones, and fill
//! the descriptions and types the others lack, at their own lines.

use cogitator_lsp::{Position, Range, TextEdit};
use cogitator_schemas::ColumnEntry;

use super::super::code_actions::yaml_text::{
    entry_at, insert_after_line, key_line, lines_range, Entry,
};
use super::create::inherited;
use super::entry::Work;
use super::inherit::Lineages;
use super::text::{apply, quoted, type_scalar};
use super::{ModelSummary, Typed};

/// The column item whose `name:` is on `line`, when it is a block-style
/// item whose keys `name:` belongs to.
pub(super) fn column_item_at(lines: &[&str], line: usize) -> Result<Entry, &'static str> {
    let entry = entry_at(lines, line).ok_or("no list item holds it")?;
    let dash_line = lines[entry.dash_line];
    if dash_line.contains('{') {
        return Err("written in flow style (`- {name: …}`), which line edits would break");
    }
    if key_line(lines, entry, "name") != Some(line) {
        return Err("its item could not be told apart from a nested list");
    }
    Ok(entry)
}

pub(super) fn prune(text: &mut String, stale: &[ColumnEntry], summary: &mut ModelSummary) {
    let lines: Vec<&str> = text.lines().collect();
    let mut edits = Vec::new();
    for column in stale {
        match column_item_at(&lines, column.name_pos.line as usize) {
            Ok(entry) => {
                edits.push(TextEdit {
                    range: lines_range(text, entry.dash_line, entry.end),
                    new_text: String::new(),
                });
                summary.pruned.push(column.name.clone());
            }
            Err(reason) => summary.skip(Some(&column.name), format!("not pruned: {reason}")),
        }
    }
    *text = apply(text, &edits);
}

/// Descriptions and types of the written columns that lack them.
pub(super) fn fill(
    work: &Work<'_>,
    lineages: &mut Lineages<'_>,
    columns: &[ColumnEntry],
    typed: &dyn Fn(&str) -> Option<String>,
    text: &mut String,
    summary: &mut ModelSummary,
) {
    let lines: Vec<&str> = text.lines().collect();
    let mut edits = Vec::new();
    for column in columns {
        let name = column.name.as_str();
        let needs_description = column.description.trim().is_empty();
        let description = if needs_description {
            inherited(work, lineages, name, summary)
        } else {
            None
        };
        let data_type = column.data_type.is_none().then(|| typed(name)).flatten();
        if needs_description && description.is_none() {
            summary.undocumented.push(column.name.clone());
        }
        if description.is_none() && data_type.is_none() {
            continue;
        }
        let line = column.name_pos.line as usize;
        let entry = match column_item_at(&lines, line) {
            Ok(entry) => entry,
            Err(reason) => {
                summary.skip(Some(name), reason);
                summary.inherited.retain(|i| i.column != name);
                continue;
            }
        };
        let pad = " ".repeat(entry.key_indent);
        let mut inserted = String::new();
        let mut keyed = |key: &str, value: String, edits: &mut Vec<TextEdit>| -> bool {
            match key_line(&lines, entry, key) {
                None => {
                    inserted.push_str(&format!("{pad}{key}: {value}\n"));
                    true
                }
                Some(at) => match empty_value(lines[at], key) {
                    Some(range) => {
                        edits.push(TextEdit {
                            range: shift(range, at),
                            new_text: format!(" {value}"),
                        });
                        true
                    }
                    None => false,
                },
            }
        };
        if let Some(description) = description {
            if !keyed("description", quoted(&description), &mut edits) {
                summary.skip(
                    Some(name),
                    "its empty description is written in a form the sync does not edit",
                );
                summary.inherited.retain(|i| i.column != name);
                summary.undocumented.push(column.name.clone());
            }
        }
        if let Some(data_type) = data_type {
            if keyed("data_type", type_scalar(&data_type), &mut edits) {
                summary.typed.push(Typed {
                    column: column.name.clone(),
                    data_type,
                });
            } else {
                summary.skip(
                    Some(name),
                    "its `data_type:` holds a value that is not a type name",
                );
            }
        }
        if !inserted.is_empty() {
            edits.push(insert_after_line(text, line, inserted));
        }
    }
    *text = apply(text, &edits);
}

/// The value span after `key:` on `line` when it says nothing (empty,
/// `""`, `''`, `~`, `null`), as a range on line 0; a trailing comment stays.
fn empty_value(line: &str, key: &str) -> Option<Range> {
    let at = line.find(&format!("{key}:"))? + key.len() + 1;
    let rest = &line[at..];
    let value_end = rest.find(" #").unwrap_or(rest.len());
    let value = rest[..value_end].trim();
    if !matches!(value, "" | "\"\"" | "''" | "~" | "null") {
        return None;
    }
    let units = |s: &str| s.encode_utf16().count() as u32;
    let start = units(&line[..at]);
    let end = units(&line[..at + rest[..value_end].trim_end().len()]);
    Some(Range {
        start: Position {
            line: 0,
            character: start,
        },
        end: Position {
            line: 0,
            character: end,
        },
    })
}

fn shift(range: Range, line: usize) -> Range {
    let at = |p: Position| Position {
        line: line as u32,
        character: p.character,
    };
    Range {
        start: at(range.start),
        end: at(range.end),
    }
}
