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

impl ExtractEdit {
    /// The edit itself — one truth for the `dbt.extractModelYaml`
    /// command and the edit-bearing code action.
    pub fn workspace_edit(&self) -> cogitator_lsp::WorkspaceEdit {
        use cogitator_lsp::{CreateFile, DocumentChange, TextDocumentEdit, TextEdit};
        let document_changes = match self {
            ExtractEdit::CreateFile { path, content } => {
                let uri = cogitator_lsp::fs_path_to_uri(path);
                vec![
                    DocumentChange::Create(CreateFile::new(uri.clone())),
                    DocumentChange::Edit(TextDocumentEdit::new(
                        uri,
                        vec![TextEdit {
                            range: Range::default(),
                            new_text: content.clone(),
                        }],
                    )),
                ]
            }
            ExtractEdit::AddVersion {
                schema_path, edits, ..
            } => vec![DocumentChange::Edit(TextDocumentEdit::new(
                cogitator_lsp::fs_path_to_uri(schema_path),
                edits
                    .iter()
                    .map(|(range, text)| TextEdit {
                        range: *range,
                        new_text: text.clone(),
                    })
                    .collect(),
            ))],
        };
        cogitator_lsp::WorkspaceEdit { document_changes }
    }
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
            let sql = std::fs::read_to_string(&details.uri).unwrap_or_default();
            content.push_str(&config_yaml(&sql, "    "));
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

/// The constant arguments of the model's `{{ config(...) }}` calls as a
/// `config:` block at `indent` (later calls win per key, as in dbt), or
/// nothing when there are none. What is not constant stays in the SQL
/// alone; what is copied keeps working there, as the SQL's config wins
/// over the YAML's.
pub fn config_yaml(sql: &str, indent: &str) -> String {
    use cogitator_sql_front::config_values;
    let mut merged: Vec<(String, cogitator_sql_front::ConfigValue)> = Vec::new();
    for call in crate::util::config_calls(sql) {
        for (key, value) in config_values(&call) {
            match merged.iter_mut().find(|(k, _)| *k == key) {
                Some(slot) => slot.1 = value,
                None => merged.push((key, value)),
            }
        }
    }
    if merged.is_empty() {
        return String::new();
    }
    let mut out = format!("{indent}config:\n");
    let inner = format!("{indent}  ");
    for (key, value) in &merged {
        yaml_entry(&mut out, &inner, key, value);
    }
    out
}

fn yaml_entry(out: &mut String, indent: &str, key: &str, value: &cogitator_sql_front::ConfigValue) {
    use cogitator_sql_front::ConfigValue;
    let key = yaml_scalar(key);
    match value {
        ConfigValue::Map(entries) if !entries.is_empty() => {
            out.push_str(&format!("{indent}{key}:\n"));
            let inner = format!("{indent}  ");
            for (k, v) in entries {
                yaml_entry(out, &inner, k, v);
            }
        }
        ConfigValue::List(items)
            if items
                .iter()
                .any(|i| matches!(i, ConfigValue::Map(_) | ConfigValue::List(_))) =>
        {
            // a list of indexes, of grants…: an item a line, each in flow
            out.push_str(&format!("{indent}{key}:\n"));
            for item in items {
                out.push_str(&format!("{indent}  - {}\n", flow(item)));
            }
        }
        other => out.push_str(&format!("{indent}{key}: {}\n", flow(other))),
    }
}

/// A value on one line: scalars plain where YAML reads them back as the
/// same string, lists and maps in flow style.
fn flow(value: &cogitator_sql_front::ConfigValue) -> String {
    use cogitator_sql_front::ConfigValue;
    match value {
        ConfigValue::Str(s) => yaml_scalar(s),
        ConfigValue::Bool(b) => b.to_string(),
        ConfigValue::Number(n) => n.clone(),
        ConfigValue::List(items) => {
            format!(
                "[{}]",
                items.iter().map(flow).collect::<Vec<_>>().join(", ")
            )
        }
        ConfigValue::Map(entries) => format!(
            "{{{}}}",
            entries
                .iter()
                .map(|(k, v)| format!("{}: {}", yaml_scalar(k), flow(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// A string plain when it is a bare word YAML would not read as anything
/// else, else double-quoted (JSON's quoting is YAML's).
fn yaml_scalar(s: &str) -> String {
    const KEYWORDS: &[&str] = &[
        "true", "false", "yes", "no", "on", "off", "null", "y", "n", "~",
    ];
    let bare = !s.is_empty()
        && s.chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/'))
        && !KEYWORDS.contains(&s.to_ascii_lowercase().as_str());
    if bare {
        s.to_string()
    } else {
        serde_json::to_string(s).unwrap_or_else(|_| format!("\"{s}\""))
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

#[cfg(test)]
mod tests {
    use super::config_yaml;
    use pretty_assertions::assert_eq;
    use rstest::rstest;
    use saphyr::{LoadableYamlNode, MarkedYaml};

    #[rstest]
    #[case::none("select 1", "")]
    #[case::materialized(
        "{{ config(materialized='incremental') }}\nselect 1",
        "    config:\n      materialized: incremental\n"
    )]
    #[case::lists_and_maps(
        "{{ config(unique_key=['id', 'day'], contract={'enforced': true}, tags='nightly') }}",
        "    config:\n      unique_key: [id, day]\n      contract:\n        enforced: true\n      tags: nightly\n"
    )]
    #[case::numbers_and_keywords(
        "{{ config(batch_size=500, on_schema_change='yes', alias='a b') }}",
        "    config:\n      batch_size: 500\n      on_schema_change: \"yes\"\n      alias: \"a b\"\n"
    )]
    #[case::later_call_wins(
        "{{ config(materialized='view') }}\n{{ config(materialized='table', schema='mart') }}",
        "    config:\n      materialized: table\n      schema: mart\n"
    )]
    #[case::not_constant_stays_in_the_sql(
        "{{ config(materialized='table', schema=var('s'), tags=['a', var('t')]) }}",
        "    config:\n      materialized: table\n"
    )]
    #[case::commented_out("{# {{ config(materialized='table') }} #}\nselect 1", "")]
    #[case::list_of_maps(
        "{{ config(indexes=[{'columns': ['id'], 'unique': true}]) }}",
        "    config:\n      indexes:\n        - {columns: [id], unique: true}\n"
    )]
    fn the_config_block(#[case] sql: &str, #[case] expected: &str) {
        assert_eq!(config_yaml(sql, "    "), expected);
    }

    #[test]
    fn the_sql_config_as_json() {
        let sql = "{{\n    config(\n        materialized='incremental',\n        incremental_strategy='delete+insert',\n        unique_key=['venue', 'id'],\n        batch=500,\n        post_hook=[\"{{ m() }}\"]\n    )\n}}";
        let got = crate::util::sql_config(sql);
        let get = |k: &str| got.iter().find(|(key, _)| key == k).map(|(_, v)| v.clone());
        assert_eq!(
            get("incremental_strategy"),
            Some(serde_json::json!("delete+insert"))
        );
        assert_eq!(get("unique_key"), Some(serde_json::json!(["venue", "id"])));
        assert_eq!(get("batch"), Some(serde_json::json!(500)));
        assert_eq!(get("post_hook"), Some(serde_json::json!(["{{ m() }}"])));
    }

    /// What is written reads back as YAML holding the same values.
    #[test]
    fn the_config_block_is_yaml() {
        let sql = "{{ config(materialized='incremental', unique_key=['id', 'day'], \
                   contract={'enforced': true}, on_schema_change='off', alias='x: y', \
                   indexes=[{'columns': ['id'], 'unique': true}]) }}";
        let text = format!("models:\n  - name: m\n{}", config_yaml(sql, "    "));
        let docs = MarkedYaml::load_from_str(&text).expect("valid YAML");
        let json = cogitator_schemas::yaml::to_json(&docs[0]);
        let config = &json["models"][0]["config"];
        assert_eq!(config["materialized"], "incremental");
        assert_eq!(config["unique_key"], serde_json::json!(["id", "day"]));
        assert_eq!(config["contract"]["enforced"], true);
        assert_eq!(config["on_schema_change"], "off");
        assert_eq!(config["alias"], "x: y");
        assert_eq!(config["indexes"][0]["columns"], serde_json::json!(["id"]));
        assert_eq!(config["indexes"][0]["unique"], true);
    }
}
