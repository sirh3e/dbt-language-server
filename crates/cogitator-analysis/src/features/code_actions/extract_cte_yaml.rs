//! The documentation an extracted CTE takes along: the parent model's
//! documented columns that the CTE produces and the parent passes
//! through unchanged (lineage `direct`, the same one upstream column on
//! both sides) become columns of a new entry for the new model, with
//! the parent's descriptions as written (a `{{ doc() }}` stays one).
//!
//! The entry goes into the parent's properties file, right after the
//! parent's entry: the file the author already keeps for that folder.
//! Nothing is written when the parent has no entry, the CTE's lineage
//! is unclear (it does not parse on its own), or no column qualifies.

use std::path::Path;

use cogitator_lsp::{DocumentChange, TextDocumentEdit};

use super::super::yaml_sync::inherit::same_column as single_direct;
use super::super::{file_uri, DbtContext};
use super::raw_description::raw_description;
use super::yaml_text::{entry_at, indent, insert_after_line, plain_name};

/// The edit adding `model`'s entry beside `parent`'s, for the CTE body
/// `body` of the parent's SQL `parent_sql`.
pub(crate) fn carried_entry(
    ctx: &DbtContext,
    parent: &str,
    parent_sql: &cogitator_parser::ParsedDocument,
    body: &str,
    model: &str,
    read: &dyn Fn(&Path) -> Option<String>,
) -> Option<DocumentChange> {
    let details = ctx.model_detail_map.get(parent)?;
    let schema = details.schema_uri.as_ref()?;
    let documented: Vec<_> = details
        .columns
        .iter()
        .filter(|c| !c.description.trim().is_empty())
        .collect();
    if documented.is_empty() {
        return None;
    }
    let parent_lineage = crate::lineage::column_lineage(parent_sql, &ctx.dialect)?;
    let cte_doc = cogitator_parser::pipeline::parse_document(body, &ctx.dialect).ok()?;
    let cte_lineage = crate::lineage::column_lineage(&cte_doc, &ctx.dialect)?;
    let mut columns = Vec::new();
    for column in documented {
        let (Some(up), Some(cte)) = (
            single_direct(&parent_lineage, &column.name),
            single_direct(&cte_lineage, &column.name),
        ) else {
            continue;
        };
        if up == cte {
            columns.push(column);
        }
    }
    if columns.is_empty() {
        return None;
    }
    let text = read(schema)?;
    let lines: Vec<&str> = text.lines().collect();
    let entry = entry_at(&lines, details.schema_range.start.line as usize)?;
    let pad = " ".repeat(indent(lines[entry.dash_line]));
    // a blank line between entries where the file has one
    let gap = if lines
        .get(entry.end + 1)
        .is_some_and(|l| l.trim().is_empty())
    {
        "\n"
    } else {
        ""
    };
    let mut body = format!("{gap}{pad}- name: {model}\n{pad}  columns:\n");
    for column in columns {
        let name = plain_name(&column.name)?;
        let written = raw_description(&text, column.name_pos)
            .unwrap_or_else(|| column.description.trim().to_string());
        let description = serde_json::to_string(written.trim()).expect("a string serializes");
        body.push_str(&format!(
            "{pad}    - name: {name}\n{pad}      description: {description}\n"
        ));
    }
    let edit = insert_after_line(&text, entry.end, body);
    Some(DocumentChange::Edit(TextDocumentEdit::new(
        file_uri(schema),
        vec![edit],
    )))
}
