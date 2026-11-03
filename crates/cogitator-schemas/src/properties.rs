//! Properties/schema `.yml` adapter (specs/rust-port/03 §6.2, 08 §2.3–2.4).
//!
//! One saphyr parse per file feeds both the typify-generated `DbtYmlFiles`
//! type (shape source of truth) and the name-position side-table.

use std::path::Path;

use cogitator_error::Result;
use cogitator_lsp::Position;

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
    pub columns: Vec<ColumnEntry>,
    /// `tags:` and `config.tags` of the entry (tag search, dbt.projectGraph).
    pub tags: Vec<String>,
}

/// A documented column from the properties YAML (write-assist completion,
/// column hover/definition).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnEntry {
    pub name: String,
    pub description: String,
    /// Position of the column's `name:` value node in the YAML file.
    pub name_pos: Position,
}

#[derive(Debug, Clone)]
pub struct SourceEntry {
    pub name: String,
    pub name_pos: Position,
    pub description: String,
    /// Relation coordinates, raw YAML strings — `env_var()` text is kept
    /// verbatim here and resolved downstream (analysis layer).
    pub database: Option<String>,
    pub schema: Option<String>,
    pub loader: Option<String>,
    pub tables: Vec<TableEntry>,
}

#[derive(Debug, Clone)]
pub struct TableEntry {
    pub name: String,
    pub name_pos: Position,
    pub description: String,
    /// Raw, unresolved — see `SourceEntry`.
    pub identifier: Option<String>,
    pub loader: Option<String>,
    pub columns: Vec<ColumnEntry>,
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
    type ModelSide = (Position, Option<String>, Vec<String>, Vec<Position>);
    let model_positions: Vec<ModelSide> = yaml::get(&marked, "models")
        .map(|models| {
            yaml::items(models)
                .map(|m| {
                    let alias = yaml::get(m, "config")
                        .and_then(|c| yaml::get(c, "alias"))
                        .and_then(|a| yaml::as_str(a).map(str::to_string));
                    (name_position(m), alias, tags_of(m), column_positions(m))
                })
                .collect()
        })
        .unwrap_or_default();
    type TableSide = (Position, Vec<Position>);
    let source_nodes: Vec<(Position, Vec<TableSide>)> = yaml::get(&marked, "sources")
        .map(|sources| {
            yaml::items(sources)
                .map(|s| {
                    let tables = yaml::get(s, "tables")
                        .map(|t| {
                            yaml::items(t)
                                .map(|table| (name_position(table), column_positions(table)))
                                .collect()
                        })
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
            let (name_pos, alias, tags, column_pos) =
                model_positions.get(i).cloned().unwrap_or_default();
            ModelEntry {
                name: m.name.clone(),
                name_pos,
                description: m.description.clone().unwrap_or_default(),
                alias,
                columns: columns_of(&m.columns, &column_pos),
                tags,
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
                database: s.database.clone(),
                schema: s.schema.clone(),
                loader: s.loader.clone(),
                tables: s
                    .tables
                    .iter()
                    .enumerate()
                    .map(|(j, t)| {
                        let (name_pos, column_pos) =
                            table_positions.get(j).cloned().unwrap_or_default();
                        TableEntry {
                            name: t.name.clone(),
                            name_pos,
                            description: t.description.clone().unwrap_or_default(),
                            identifier: t.identifier.clone(),
                            loader: t.loader.clone(),
                            columns: columns_of(&t.columns, &column_pos),
                        }
                    })
                    .collect(),
            }
        })
        .collect();

    Ok(PropertiesFile { models, sources })
}

/// `tags:` on the entry plus `config.tags`, each a string or a list.
fn tags_of(entry: &saphyr::MarkedYaml<'_>) -> Vec<String> {
    let mut tags = Vec::new();
    let nodes = [
        yaml::get(entry, "tags"),
        yaml::get(entry, "config").and_then(|c| yaml::get(c, "tags")),
    ];
    for node in nodes.into_iter().flatten() {
        if let Some(tag) = yaml::as_str(node) {
            tags.push(tag.to_string());
        } else {
            tags.extend(yaml::items(node).filter_map(|t| yaml::as_str(t).map(str::to_string)));
        }
    }
    tags.sort();
    tags.dedup();
    tags
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
                map.retain(|k, _| matches!(k.as_str(), "name" | "description" | "columns"));
                prune_columns(map);
            }
        }
    }
    if let Some(Value::Array(sources)) = root.get_mut("sources") {
        for s in sources {
            if let Value::Object(map) = s {
                map.retain(|k, _| {
                    matches!(
                        k.as_str(),
                        "name" | "description" | "database" | "schema" | "loader" | "tables"
                    )
                });
                if let Some(Value::Array(tables)) = map.get_mut("tables") {
                    for t in tables {
                        if let Value::Object(tmap) = t {
                            tmap.retain(|k, _| {
                                matches!(
                                    k.as_str(),
                                    "name" | "description" | "identifier" | "loader" | "columns"
                                )
                            });
                            prune_columns(tmap);
                        }
                    }
                }
            }
        }
    }
}

fn columns_of(
    columns: &[crate::generated::dbt_yml_files::ColumnProperties],
    positions: &[Position],
) -> Vec<ColumnEntry> {
    columns
        .iter()
        .enumerate()
        .map(|(i, c)| ColumnEntry {
            name: c.name.clone(),
            description: c.description.clone().unwrap_or_default(),
            name_pos: positions.get(i).copied().unwrap_or_default(),
        })
        .collect()
}

/// Positions of each `columns[j].name` value node of one model/table entry.
fn column_positions(entry: &saphyr::MarkedYaml<'_>) -> Vec<Position> {
    yaml::get(entry, "columns")
        .map(|columns| yaml::items(columns).map(name_position).collect())
        .unwrap_or_default()
}

fn prune_columns(map: &mut serde_json::Map<String, serde_json::Value>) {
    use serde_json::Value;
    if let Some(Value::Array(columns)) = map.get_mut("columns") {
        for c in columns {
            if let Value::Object(cmap) = c {
                cmap.retain(|k, _| k == "name" || k == "description");
            }
        }
    }
}
