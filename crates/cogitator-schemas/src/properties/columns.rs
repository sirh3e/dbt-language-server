//! The `columns:` list of a model or source table: generated struct fields
//! joined positionally with what only the marked tree holds (08 §2.4).

use cogitator_lsp::Position;

use super::{name_position, ColumnEntry};
use crate::generated::dbt_yml_files::ColumnProperties;
use crate::yaml;

/// What the marked tree contributes to one column.
#[derive(Debug, Clone, Default)]
pub(super) struct ColumnSide {
    name_pos: Position,
    data_type: Option<String>,
}

pub(super) fn columns_of(columns: &[ColumnProperties], sides: &[ColumnSide]) -> Vec<ColumnEntry> {
    columns
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let side = sides.get(i).cloned().unwrap_or_default();
            ColumnEntry {
                name: c.name.clone(),
                description: c.description.clone().unwrap_or_default(),
                name_pos: side.name_pos,
                data_type: side.data_type,
            }
        })
        .collect()
}

/// One `ColumnSide` per `columns[j]` of a model/table entry. `data_type`
/// comes from the tree rather than the generated type, so a non-string
/// value reads as absent instead of rejecting the file.
pub(super) fn column_sides(entry: &saphyr::MarkedYaml<'_>) -> Vec<ColumnSide> {
    yaml::get(entry, "columns")
        .map(|columns| {
            yaml::items(columns)
                .map(|column| ColumnSide {
                    name_pos: name_position(column),
                    data_type: yaml::get(column, "data_type")
                        .and_then(yaml::as_str)
                        .filter(|text| !text.trim().is_empty())
                        .map(str::to_string),
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn prune_columns(map: &mut serde_json::Map<String, serde_json::Value>) {
    use serde_json::Value;
    if let Some(Value::Array(columns)) = map.get_mut("columns") {
        for c in columns {
            if let Value::Object(cmap) = c {
                cmap.retain(|k, _| k == "name" || k == "description");
            }
        }
    }
}
