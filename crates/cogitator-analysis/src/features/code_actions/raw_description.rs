//! A column's description as written in its properties file. The
//! context holds descriptions with `{{ doc('x') }}` already expanded;
//! an action that copies a description copies the written text, so a
//! doc() call is carried as the call and the doc block stays the one
//! place the words live. Falls back to the context's text where the
//! value is not a one-line scalar (a `|` or `>` block).

use std::path::Path;

use cogitator_lsp::Position;
use cogitator_schemas::yaml::{as_str, get};
use saphyr::{LoadableYamlNode, MarkedYaml};

use super::super::DbtContext;
use super::yaml_text::{entry_at, key_line};
use crate::lineage::SourceRef;

/// The one-line `description:` value of the column entry whose `name:`
/// value is at `name_pos`, parsed as YAML reads it.
pub(crate) fn raw_description(text: &str, name_pos: Position) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    let entry = entry_at(&lines, name_pos.line as usize)?;
    let at = key_line(&lines, entry, "description")?;
    let line = lines[at].trim_start().trim_start_matches("- ");
    let value = line.strip_prefix("description:")?.trim();
    if value.is_empty() || value.starts_with(['|', '>']) {
        return None;
    }
    let docs = MarkedYaml::load_from_str(&format!("d: {value}")).ok()?;
    as_str(get(docs.first()?, "d")?).map(str::to_string)
}

/// The written description of the documented column `from` names.
pub(crate) fn written(
    ctx: &DbtContext,
    from: &SourceRef,
    read: &dyn Fn(&Path) -> Option<String>,
) -> Option<String> {
    let (file, columns, column) = match from {
        SourceRef::Model { name, column } => {
            let details = ctx.model_detail_map.get(name)?;
            (details.schema_uri.as_deref()?, &details.columns, column)
        }
        SourceRef::Source {
            source,
            table,
            column,
        } => {
            let t = ctx.source_detail_map.get(source)?.tables.get(table)?;
            (t.uri.as_path(), &t.columns, column)
        }
        SourceRef::Unknown { .. } => return None,
    };
    let entry = columns
        .iter()
        .find(|c| c.name.eq_ignore_ascii_case(column))?;
    raw_description(&read(file)?, entry.name_pos).map(|d| d.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(line: u32) -> Position {
        Position { line, character: 0 }
    }

    #[test]
    fn the_written_value_is_read_as_yaml_reads_it() {
        let text = "columns:\n  - name: a\n    description: \"{{ doc('a') }}\"\n  - name: b\n    description: it's plain # note\n  - name: c\n    description: >\n      folded\n  - name: d\n    description: 'it''s quoted'\n";
        assert_eq!(
            raw_description(text, at(1)).as_deref(),
            Some("{{ doc('a') }}")
        );
        assert_eq!(raw_description(text, at(3)).as_deref(), Some("it's plain"));
        assert_eq!(raw_description(text, at(5)), None);
        assert_eq!(raw_description(text, at(8)).as_deref(), Some("it's quoted"));
    }
}
