//! The model's columns against its properties entry, from its SQL:
//!
//! - "Add missing columns": the produced columns (per lineage) the entry
//!   does not document become `- name:` items — appended to its
//!   `columns:` list, or a new list. The fix of `lint-undocumented-column`,
//!   and of `contract-drift`'s undeclared columns: under an enforced
//!   contract each item also gets the `data_type:` dbt requires, taken
//!   from the upstream column lineage names where that one declares it.
//! - "Remove contracted columns the model never produces": the other
//!   half of `contract-drift`; deletes those items. Never preferred —
//!   it is the contract that may be right.
//!
//! The edits land in the YAML file, whose text the shell reads.

use std::path::Path;

use cogitator_lsp::{CodeAction, DocumentChange, TextDocumentEdit, TextEdit, WorkspaceEdit};

use super::super::lint::undocumented::missing_columns;
use super::super::DbtContext;
use super::yaml_text::{append_to_list, block_end, indent, lines_range, plain_name};
use crate::context::ModelDetails;
use crate::lineage::{ModelLineage, SourceRef};

/// The add action for `model`, given the text of its properties file;
/// None when nothing is missing or the entry cannot be found in the text.
pub fn add_missing_columns_action(
    ctx: &DbtContext,
    model: &str,
    details: &ModelDetails,
    lineage: &ModelLineage,
    yaml_text: &str,
) -> Option<CodeAction> {
    let schema: &Path = details.schema_uri.as_deref()?;
    let missing: Vec<&str> = missing_columns(details, lineage)
        .into_iter()
        .filter_map(plain_name)
        .collect();
    if missing.is_empty() {
        return None;
    }
    let items: Vec<Vec<String>> = missing
        .iter()
        .map(|name| {
            let mut item = vec![format!("- name: {name}"), "  description: \"\"".to_string()];
            if details.contract_enforced {
                let data_type = upstream_data_type(ctx, lineage, name).unwrap_or_default();
                item.push(format!("  data_type: {}", yaml_scalar(&data_type)));
            }
            item
        })
        .collect();
    let line = details.schema_range.start.line as usize;
    let edit = append_to_list(yaml_text, line, "columns", &items)?;
    let title = if details.contract_enforced {
        format!(
            "Add {} undeclared column(s) of '{model}' to its contract",
            missing.len()
        )
    } else {
        format!(
            "Add {} missing column(s) of '{model}' to its schema YAML",
            missing.len()
        )
    };
    Some(yaml_action(title, schema, vec![edit]))
}

/// The remove action: under an enforced contract, the declared columns
/// the model no longer produces. A `*` production says nothing, as the
/// contract-drift check itself holds.
pub fn remove_unproduced_columns_action(
    model: &str,
    details: &ModelDetails,
    lineage: &ModelLineage,
    yaml_text: &str,
) -> Option<CodeAction> {
    let schema: &Path = details.schema_uri.as_deref()?;
    if !details.contract_enforced || lineage.columns.iter().any(|c| c.name.contains('*')) {
        return None;
    }
    let lines: Vec<&str> = yaml_text.lines().collect();
    let mut edits = Vec::new();
    for column in &details.columns {
        let produced = lineage
            .columns
            .iter()
            .any(|c| c.name.eq_ignore_ascii_case(&column.name));
        if produced {
            continue;
        }
        let line = column.name_pos.line as usize;
        let item = lines.get(line)?;
        if !item.trim_start().starts_with('-') {
            return None; // `name:` is not the item's first key: hands off
        }
        let end = block_end(&lines, line, indent(item));
        edits.push(TextEdit {
            range: lines_range(yaml_text, line, end),
            new_text: String::new(),
        });
    }
    if edits.is_empty() {
        return None;
    }
    let title = format!(
        "Remove {} contracted column(s) '{model}' never produces",
        edits.len()
    );
    Some(yaml_action(title, schema, edits))
}

/// The `data_type:` the upstream column declares, when lineage traces the
/// column to exactly one model column that has one.
fn upstream_data_type(ctx: &DbtContext, lineage: &ModelLineage, column: &str) -> Option<String> {
    let col = lineage
        .columns
        .iter()
        .find(|c| c.name.eq_ignore_ascii_case(column))?;
    let [SourceRef::Model {
        name,
        column: upstream,
    }] = col.sources.as_slice()
    else {
        return None;
    };
    ctx.model_detail_map
        .get(name)?
        .columns
        .iter()
        .find(|c| c.name.eq_ignore_ascii_case(upstream))?
        .data_type
        .clone()
}

/// A type as written (`varchar`, `decimal(18, 2)`), quoted only when YAML
/// would read it otherwise; empty stays an explicit `""` for the reader.
fn yaml_scalar(value: &str) -> String {
    let plain = !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || " _(),".contains(c));
    if plain {
        value.to_string()
    } else {
        format!("\"{}\"", value.replace('"', "'"))
    }
}

fn yaml_action(title: String, schema: &Path, edits: Vec<TextEdit>) -> CodeAction {
    CodeAction {
        title,
        kind: Some("quickfix"),
        diagnostics: None,
        edit: Some(WorkspaceEdit {
            document_changes: vec![DocumentChange::Edit(TextDocumentEdit::new(
                cogitator_lsp::fs_path_to_uri(schema),
                edits,
            ))],
        }),
        command: None,
        is_preferred: None,
        data: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cogitator_lsp::Position;

    const YAML: &str = "version: 2\n\nmodels:\n  - name: a\n    description: x\n\n    columns:\n      - name: id\n        description: key\n\n  - name: b\n    description: y\n";

    fn item(name: &str) -> Vec<String> {
        vec![format!("- name: {name}"), "  description: \"\"".into()]
    }

    #[test]
    fn appends_to_an_existing_columns_list() {
        let edit = append_to_list(YAML, 3, "columns", &[item("total")]).unwrap();
        assert_eq!(
            edit.range.start,
            Position {
                line: 9,
                character: 0
            }
        );
        assert_eq!(
            edit.new_text,
            "      - name: total\n        description: \"\"\n"
        );
    }

    #[test]
    fn starts_a_columns_list_where_the_entry_has_none() {
        let edit = append_to_list(YAML, 10, "columns", &[item("id"), item("n")]).unwrap();
        assert_eq!(
            edit.range.start,
            Position {
                line: 12,
                character: 0
            }
        );
        assert_eq!(
            edit.new_text,
            "    columns:\n      - name: id\n        description: \"\"\n      - name: n\n        description: \"\"\n"
        );
    }

    #[test]
    fn an_inline_columns_value_is_left_alone() {
        let yaml = "models:\n  - name: a\n    columns: []\n";
        assert!(append_to_list(yaml, 1, "columns", &[item("id")]).is_none());
    }

    #[test]
    fn types_are_written_plain_where_yaml_reads_them_so() {
        assert_eq!(yaml_scalar("varchar"), "varchar");
        assert_eq!(yaml_scalar("decimal(18, 2)"), "decimal(18, 2)");
        assert_eq!(yaml_scalar(""), "\"\"");
        assert_eq!(yaml_scalar("struct<a: int>"), "\"struct<a: int>\"");
    }
}
