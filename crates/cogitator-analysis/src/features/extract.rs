//! dbt.extractModelYaml: scaffold the properties YAML for the model under
//! the cursor — a new `<model>.yml` with its columns when no schema entry
//! exists, or the next `versions:` entry when one does. The dispatcher
//! sends the result to the client as a workspace/applyEdit.

use std::path::PathBuf;

use cogitator_lsp::{Position, Range};
use cogitator_schemas::yaml::{as_str, entries, get, items, position_of, to_json};
use saphyr::{LoadableYamlNode, MarkedYaml};

use super::{model_name_from_uri, DbtContext};
use crate::lineage::ModelLineage;

pub enum ExtractEdit {
    /// No schema entry yet: create `<model>.yml` beside the model.
    CreateFile { path: PathBuf, content: String },
    /// Entry exists: add the next version to the schema file.
    AddVersion {
        schema_path: PathBuf,
        edits: Vec<(Range, String)>,
        next: i64,
    },
}

/// Column names for the scaffold: lineage first (the real select list),
/// then the already-documented YAML columns, else none.
fn column_names(ctx: &DbtContext, model: &str, lineage: Option<&ModelLineage>) -> Vec<String> {
    if let Some(lineage) = lineage {
        if !lineage.columns.is_empty() {
            return lineage.columns.iter().map(|c| c.name.clone()).collect();
        }
    }
    ctx.model_detail_map
        .get(model)
        .map(|d| d.columns.iter().map(|c| c.name.clone()).collect())
        .unwrap_or_default()
}

pub fn extract_model_yaml(
    ctx: &DbtContext,
    uri: &str,
    lineage: Option<&ModelLineage>,
) -> Option<ExtractEdit> {
    let model = model_name_from_uri(uri)?;
    let details = ctx.model_detail_map.get(&model)?;

    match &details.schema_uri {
        None => {
            let path = details.uri.with_file_name(format!("{model}.yml"));
            if path.exists() {
                return None; // a file is there but holds no entry — hands off
            }
            let mut content =
                format!("version: 2\n\nmodels:\n  - name: {model}\n    description: \"\"\n");
            let columns = column_names(ctx, &model, lineage);
            if !columns.is_empty() {
                content.push_str("    columns:\n");
                for column in columns {
                    content.push_str(&format!(
                        "      - name: {column}\n        description: \"\"\n"
                    ));
                }
            }
            Some(ExtractEdit::CreateFile { path, content })
        }
        Some(schema_path) => {
            let text = std::fs::read_to_string(schema_path).ok()?;
            let edits = version_edits(&text, &model)?;
            Some(ExtractEdit::AddVersion {
                schema_path: schema_path.clone(),
                edits: edits.0,
                next: edits.1,
            })
        }
    }
}

/// Text edits that add the next `versions:` entry (and keep
/// `latest_version` current) to the model's existing schema entry.
fn version_edits(text: &str, model: &str) -> Option<(Vec<(Range, String)>, i64)> {
    let docs = MarkedYaml::load_from_str(text).ok()?;
    let root = docs.first()?;
    let entry =
        items(get(root, "models")?).find(|m| get(m, "name").and_then(as_str) == Some(model))?;
    let name_key = entries(entry).find(|(k, _)| as_str(k) == Some("name"))?.0;
    let key_pos = position_of(name_key);
    let indent = " ".repeat(key_pos.character as usize);

    let insert_at = |line: u32| Range::zero_width(Position { line, character: 0 });
    let mut edits = Vec::new();

    match get(entry, "versions") {
        None => {
            // right under the name line: latest_version + versions block
            edits.push((
                insert_at(key_pos.line + 1),
                format!("{indent}latest_version: 1\n{indent}versions:\n{indent}  - v: 1\n"),
            ));
            Some((edits, 1))
        }
        Some(versions) => {
            let mut max_v = 0i64;
            let mut last: Option<&MarkedYaml> = None;
            for item in items(versions) {
                if let Some(v) = get(item, "v").map(to_json).and_then(|v| v.as_i64()) {
                    max_v = max_v.max(v);
                }
                last = Some(item);
            }
            let last = last?;
            let next = max_v + 1;
            let item_pos = position_of(last);
            let dash_indent = " ".repeat((item_pos.character as usize).saturating_sub(2));
            edits.push((
                insert_at(item_pos.line + 1),
                format!("{dash_indent}- v: {next}\n"),
            ));
            if let Some(latest) = entries(entry)
                .find(|(k, _)| as_str(k) == Some("latest_version"))
                .map(|(_, v)| v)
            {
                let pos = position_of(latest);
                let old_len = to_json(latest)
                    .as_i64()
                    .map(|v| v.to_string().len())
                    .unwrap_or(0) as u32;
                edits.push((
                    Range {
                        start: pos,
                        end: Position {
                            line: pos.line,
                            character: pos.character + old_len,
                        },
                    },
                    next.to_string(),
                ));
            } else {
                edits.push((
                    insert_at(key_pos.line + 1),
                    format!("{indent}latest_version: {next}\n"),
                ));
            }
            Some((edits, next))
        }
    }
}
