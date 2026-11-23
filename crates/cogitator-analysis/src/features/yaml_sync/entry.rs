//! One model's sync over the text of its properties file, in phases:
//! prune, fill (descriptions and types of the written columns), add
//! (missing columns), reorder. Each phase reads the entry from the text
//! as it stands, so the next one never edits at stale positions.

use std::path::Path;

use cogitator_schemas::{parse_properties, ColumnEntry};

use super::super::code_actions::yaml_text::{append_to_list, entry_at, key_line};
use super::create::{column_item, new_entry};
use super::fill::{fill, prune};
use super::inherit::Lineages;
use super::text::apply;
use super::{order, ModelSummary, SyncOptions};
use crate::features::column_types::ModelColumnTypes;
use crate::DbtContext;

pub(super) struct Work<'a> {
    pub ctx: &'a DbtContext,
    pub model: &'a str,
    pub path: &'a Path,
    pub types: &'a dyn Fn(&str) -> Option<ModelColumnTypes>,
    pub options: &'a SyncOptions,
}

/// What the model produces, per its lineage.
pub(super) struct Produced {
    /// Named output columns, in order.
    pub columns: Vec<String>,
    /// No `*` is left unexpanded: the list is the model's whole output.
    pub complete: bool,
}

/// Where the entry stands in the text: its `name:` line and columns.
struct Found {
    name_line: usize,
    columns: Vec<ColumnEntry>,
}

fn find_entry(text: &str, path: &Path, model: &str) -> Result<Option<Found>, String> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    let parsed = parse_properties(text, path)
        .map_err(|e| format!("the properties file does not parse: {e}"))?;
    Ok(parsed
        .models
        .into_iter()
        .find(|m| m.name == model)
        .map(|m| Found {
            name_line: m.name_pos.line as usize,
            columns: m.columns,
        }))
}

pub(super) fn sync_model(
    work: &Work<'_>,
    lineages: &mut Lineages<'_>,
    text: &mut String,
    summary: &mut ModelSummary,
) {
    let Some(lineage) = lineages.of(work.model) else {
        summary.skip(
            None,
            "its column lineage is unclear: Jinja control flow without fresh \
             compiled SQL (run dbt compile), or SQL that does not parse",
        );
        return;
    };
    let produced = Produced {
        columns: distinct(lineage.columns.iter().map(|c| c.name.as_str())),
        complete: !lineage.columns.iter().any(|c| c.name.contains('*')),
    };
    if !produced.complete {
        summary.skip(
            None,
            "a `*` whose columns are not known (its upstream has no closed \
             schema): stale columns are not reported and nothing is pruned or reordered",
        );
    }
    let types = if work.options.inherit_types {
        (work.types)(work.model)
    } else {
        None
    };
    let typed = |column: &str| -> Option<String> {
        let t = types.as_ref()?.column(column)?;
        t.is_proven().then(|| t.data_type.clone()).flatten()
    };
    let found = match find_entry(text, work.path, work.model) {
        Ok(found) => found,
        Err(reason) => {
            summary.skip(None, reason);
            return;
        }
    };
    let Some(found) = found else {
        new_entry(work, lineages, &produced, &typed, text, summary);
        return;
    };
    if has_versions(text, found.name_line) {
        summary.skip(
            None,
            "a versioned model: its columns live per version, which the sync does not edit",
        );
        return;
    }
    // prune
    if produced.complete {
        let stale: Vec<ColumnEntry> = found
            .columns
            .iter()
            .filter(|c| !contains(&produced.columns, &c.name))
            .cloned()
            .collect();
        summary.stale = stale.iter().map(|c| c.name.clone()).collect();
        if work.options.prune && !stale.is_empty() {
            prune(text, &stale, summary);
        }
    }
    // fill
    let Some(found) = reread(work, text, summary) else {
        return;
    };
    fill(work, lineages, &found.columns, &typed, text, summary);
    // add
    let Some(found) = reread(work, text, summary) else {
        return;
    };
    let missing: Vec<&String> = produced
        .columns
        .iter()
        .filter(|c| !found.columns.iter().any(|w| w.name.eq_ignore_ascii_case(c)))
        .collect();
    if !missing.is_empty() {
        let items: Vec<Vec<String>> = missing
            .iter()
            .map(|c| column_item(work, lineages, c, &typed, summary))
            .collect();
        match append_to_list(text, found.name_line, "columns", &items) {
            Some(edit) => {
                *text = apply(text, &[edit]);
                summary.added = missing.iter().map(|c| c.to_string()).collect();
            }
            None => {
                summary
                    .inherited
                    .retain(|i| !contains_ref(&missing, &i.column));
                summary.typed.retain(|t| !contains_ref(&missing, &t.column));
                summary.undocumented.retain(|u| !contains_ref(&missing, u));
                summary.skip(
                    None,
                    "`columns:` is written inline; missing columns are not added",
                );
            }
        }
    }
    // reorder
    if work.options.reorder && produced.complete {
        if let Some(found) = reread(work, text, summary) {
            if let Some(new_text) = order::reorder(text, &found.columns, &produced.columns) {
                *text = new_text;
                summary.reordered = true;
            }
        }
    }
}

/// Named output columns, each once (a repeated name is one column).
fn distinct<'a>(names: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for name in names.filter(|n| !n.contains('*')) {
        if !contains(&out, name) {
            out.push(name.to_string());
        }
    }
    out
}

fn contains(names: &[String], name: &str) -> bool {
    names.iter().any(|n| n.eq_ignore_ascii_case(name))
}

fn contains_ref(names: &[&String], name: &str) -> bool {
    names.iter().any(|n| n.eq_ignore_ascii_case(name))
}

/// The entry again after a phase; a text the phase broke is a bug, said.
fn reread(work: &Work<'_>, text: &str, summary: &mut ModelSummary) -> Option<Found> {
    match find_entry(text, work.path, work.model) {
        Ok(Some(found)) => Some(found),
        _ => {
            summary.skip(None, "the entry could not be read back after an edit");
            None
        }
    }
}

/// The entry holding `name_line` has its own `versions:` key.
fn has_versions(text: &str, name_line: usize) -> bool {
    let lines: Vec<&str> = text.lines().collect();
    entry_at(&lines, name_line).is_some_and(|e| key_line(&lines, e, "versions").is_some())
}
