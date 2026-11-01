//! Properties/schema `.yml` adapter (specs/rust-port/03 §6.2, 08 §2.3–2.4).
//!
//! One saphyr parse per file feeds both the typify-generated `DbtYmlFiles`
//! type (shape source of truth) and the name-position side-table.

use std::path::Path;

use dbt_ls_error::Result;
use dbt_ls_lsp::Position;

use crate::generated::dbt_yml_files::DbtYmlFiles;
use crate::yaml;

#[derive(Debug, Clone, Default)]
pub struct PropertiesFile {
    pub models: Vec<ModelEntry>,
    pub sources: Vec<SourceEntry>,
}

#[derive(Debug, Clone)]
pub struct ModelEntry {
    pub name: String,
    pub name_pos: Position,
    pub description: String,
    pub alias: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SourceEntry {
    pub name: String,
    pub name_pos: Position,
    pub description: String,
    pub tables: Vec<TableEntry>,
}

#[derive(Debug, Clone)]
pub struct TableEntry {
    pub name: String,
    pub name_pos: Position,
    pub description: String,
}

/// Parse one properties YAML file. Failures degrade per file (Go returned an
/// empty `PropertiesYaml` on decode errors).
pub fn parse_properties(text: &str, path: &Path) -> Result<PropertiesFile> {
    let marked = yaml::load_marked(text, path)?;
    let mut value = yaml::to_json(&marked);
    // Projection before deserialization, same rationale as in project.rs:
    // the server reads names/descriptions/tables only (03 §6.2); columns,
    // tests and configs are dropped so a strict generated union cannot
    // reject an otherwise fine file.
    prune_for_server(&mut value);
    let parsed: DbtYmlFiles = serde_json::from_value(value)?;

    // Side-table: positions of the `name:` *value* nodes, keyed by list order
    // so entries join positionally with the generated structs. The alias also
    // comes from the tree: dbt-jsonschema does not model `config.alias` on
    // properties-file models, but dbt (and the Go server) honor it (03 §6.3).
    let model_positions: Vec<(Position, Option<String>)> = yaml::get(&marked, "models")
        .map(|models| {
            yaml::items(models)
                .map(|m| {
                    let alias = yaml::get(m, "config")
                        .and_then(|c| yaml::get(c, "alias"))
                        .and_then(|a| yaml::as_str(a).map(str::to_string));
                    (name_position(m), alias)
                })
                .collect()
        })
        .unwrap_or_default();
    let source_nodes: Vec<(Position, Vec<Position>)> = yaml::get(&marked, "sources")
        .map(|sources| {
            yaml::items(sources)
                .map(|s| {
                    let tables = yaml::get(s, "tables")
                        .map(|t| yaml::items(t).map(name_position).collect())
                        .unwrap_or_default();
                    (name_position(s), tables)
                })
                .collect()
        })
        .unwrap_or_default();

    let models = parsed
        .models
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let (name_pos, alias) = model_positions.get(i).cloned().unwrap_or_default();
            ModelEntry {
                name: m.name.clone(),
                name_pos,
                description: m.description.clone().unwrap_or_default(),
                alias,
            }
        })
        .collect();

    let sources = parsed
        .sources
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let (source_pos, table_positions) = source_nodes
                .get(i)
                .cloned()
                .unwrap_or((Position::default(), Vec::new()));
            SourceEntry {
                name: s.name.clone(),
                name_pos: source_pos,
                description: s.description.clone().unwrap_or_default(),
                tables: s
                    .tables
                    .iter()
                    .enumerate()
                    .map(|(j, t)| TableEntry {
                        name: t.name.clone(),
                        name_pos: table_positions.get(j).copied().unwrap_or_default(),
                        description: t.description.clone().unwrap_or_default(),
                    })
                    .collect(),
            }
        })
        .collect();

    Ok(PropertiesFile { models, sources })
}

fn name_position(entry: &saphyr::MarkedYaml<'_>) -> Position {
    yaml::get(entry, "name")
        .map(|n| yaml::position_of(n))
        .unwrap_or_default()
}

fn prune_for_server(value: &mut serde_json::Value) {
    use serde_json::Value;
    let Value::Object(root) = value else { return };
    root.retain(|k, _| k == "models" || k == "sources" || k == "version");
    if let Some(Value::Array(models)) = root.get_mut("models") {
        for m in models {
            if let Value::Object(map) = m {
                map.retain(|k, _| k == "name" || k == "description");
            }
        }
    }
    if let Some(Value::Array(sources)) = root.get_mut("sources") {
        for s in sources {
            if let Value::Object(map) = s {
                map.retain(|k, _| {
                    matches!(
                        k.as_str(),
                        "name" | "description" | "database" | "schema" | "tables"
                    )
                });
                if let Some(Value::Array(tables)) = map.get_mut("tables") {
                    for t in tables {
                        if let Value::Object(tmap) = t {
                            tmap.retain(|k, _| k == "name" || k == "description");
                        }
                    }
                }
            }
        }
    }
}
