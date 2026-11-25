//! New YAML: an item for a produced column the entry lacks, and an entry
//! for a model that has none, each with what it inherits.

use super::entry::{Produced, Work};
use super::inherit::{walk, Lineages, Walk};
use super::text::{append_top_level, name_scalar, quoted, type_scalar};
use super::{Inherited, ModelSummary, Typed};

/// The inherited description of `column`, recorded in the summary.
pub(super) fn inherited(
    work: &Work<'_>,
    lineages: &mut Lineages<'_>,
    column: &str,
    summary: &mut ModelSummary,
) -> Option<String> {
    match walk(work.ctx, lineages, work.model, column) {
        Walk::Found(found) => {
            summary.inherited.push(Inherited {
                column: column.to_string(),
                from: found.from,
                via: found.via,
            });
            Some(found.description)
        }
        Walk::Nothing => None,
        Walk::Unclear(reason) => {
            summary.skip(Some(column), format!("no description inherited: {reason}"));
            None
        }
    }
}

/// A new `- name:` item for a produced column, with what it inherits.
pub(super) fn column_item(
    work: &Work<'_>,
    lineages: &mut Lineages<'_>,
    column: &str,
    typed: &dyn Fn(&str) -> Option<String>,
    summary: &mut ModelSummary,
) -> Vec<String> {
    let description = inherited(work, lineages, column, summary);
    if description.is_none() {
        summary.undocumented.push(column.to_string());
    }
    let mut item = vec![
        format!("- name: {}", name_scalar(column)),
        format!(
            "  description: {}",
            quoted(description.as_deref().unwrap_or(""))
        ),
    ];
    if let Some(data_type) = typed(column) {
        item.push(format!("  data_type: {}", type_scalar(&data_type)));
        summary.typed.push(Typed {
            column: column.to_string(),
            data_type,
        });
    }
    item
}

/// A model without an entry: one, with every produced column.
pub(super) fn new_entry(
    work: &Work<'_>,
    lineages: &mut Lineages<'_>,
    produced: &Produced,
    typed: &dyn Fn(&str) -> Option<String>,
    text: &mut String,
    summary: &mut ModelSummary,
) {
    let mut item = vec![
        format!("- name: {}", name_scalar(work.model)),
        "  description: \"\"".to_string(),
    ];
    if !produced.columns.is_empty() {
        item.push("  columns:".to_string());
        for column in &produced.columns {
            for line in column_item(work, lineages, column, typed, summary) {
                item.push(format!("    {line}"));
            }
        }
    }
    let base = if text.trim().is_empty() {
        "version: 2\n".to_string()
    } else {
        text.clone() // boundary: the phase's new text
    };
    match append_top_level(&base, "models", &[item]) {
        Some(new_text) => {
            *text = new_text;
            summary.created_entry = true;
            summary.added = produced.columns.clone();
        }
        None => {
            summary.inherited.clear();
            summary.typed.clear();
            summary.undocumented.clear();
            summary.skip(None, "`models:` of the properties file is written inline");
        }
    }
}
