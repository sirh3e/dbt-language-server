//! Models, seeds, sources and `{% docs %}` expansion (03 §6), served from
//! the scan cache: unchanged YAML/markdown files are not re-read or
//! re-parsed; only the cheap docs-expansion runs per refresh.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use cogitator_lsp::{Position, Range};
use cogitator_parser::jinja_scan::{self, RawDocsBlock};
use cogitator_schemas::project::ProjectFile;
use cogitator_schemas::properties::{parse_properties, PropertiesFile, SourceEntry};

use crate::cache::FileCache;
use crate::util;

use super::{ModelDetails, ModelKind, ProjectDetails, SourceDetails, SourceTableDetails};

pub(super) type PropsCache = FileCache<PropertiesFile>;
pub(super) type DocsCache = FileCache<Vec<RawDocsBlock>>;

pub(super) type SqlTagsCache = FileCache<Vec<String>>;
pub(super) type SnapshotsCache = FileCache<Vec<jinja_scan::RawSnapshot>>;

pub(super) fn build_model_details(
    root: &Path,
    project: &ProjectFile,
    process_list: &[ProjectDetails],
    props: &mut PropsCache,
    docs: &mut DocsCache,
    sql_tags: &mut SqlTagsCache,
    snapshots: &mut SnapshotsCache,
) -> (
    HashMap<String, ModelDetails>,
    HashMap<String, SourceDetails>,
    HashMap<String, super::docs::DocDetails>,
) {
    let mut models = HashMap::new();
    let mut sources = HashMap::new();
    let mut doc_details: HashMap<String, super::docs::DocDetails> = HashMap::new();

    for p in process_list {
        let path_map = util::create_file_name_map(&[".sql", ".py"], &p.root, &p.file.model_paths);
        let docs_map = super::docs::build_doc_details(&p.root, &p.file, docs);
        let (schema_models, project_sources) = schema_entries(&p.root, &p.file, &docs_map, props);
        // first definition wins: the own project is scanned first
        for (name, details) in docs_map {
            doc_details.entry(name).or_insert(details);
        }

        sources.extend(project_sources);

        for (stem, file_path) in path_map {
            let schema = schema_models.get(&stem);
            let key = schema
                .and_then(|m| m.alias.clone())
                .filter(|a| !a.is_empty())
                .unwrap_or_else(|| stem.clone());
            // yaml tags plus `{{ config(tags=…) }}` from the SQL (cached)
            let mut tags = schema.map(|m| m.tags.clone()).unwrap_or_default();
            if let Some(from_sql) =
                sql_tags.get_or_parse(&file_path, |text, _| Some(util::config_tags(text)))
            {
                tags.extend(from_sql.iter().cloned());
            }
            tags.sort();
            tags.dedup();
            models.insert(
                key,
                ModelDetails {
                    kind: ModelKind::Model,
                    uri: file_path,
                    project_name: p.file.name.clone(),
                    description: schema.map(|m| m.description.clone()).unwrap_or_default(),
                    schema_uri: schema.map(|m| m.schema_uri.clone()),
                    schema_range: schema
                        .map(|m| Range::zero_width(m.name_pos))
                        .unwrap_or_default(),
                    columns: schema.map(|m| m.columns.clone()).unwrap_or_default(),
                    tags,
                    access: schema.and_then(|m| m.access.clone()),
                    group: schema.and_then(|m| m.group.clone()),
                    contract_enforced: schema.is_some_and(|m| m.contract_enforced),
                },
            );
        }
    }

    // Seeds (own project only), overriding on collision (03 §6.3).
    let seed_map = util::create_file_name_map(&[".csv"], root, &project.seed_paths);
    for (stem, file_path) in seed_map {
        models.insert(
            stem,
            ModelDetails {
                kind: ModelKind::Seed,
                uri: file_path,
                project_name: project.name.clone(),
                description: "Seed File".to_string(),
                ..Default::default()
            },
        );
    }

    // Snapshots (03 §6 addendum): `{% snapshot name %}` blocks are
    // ref()-able by block name, never by file stem. A name collision
    // with a real model keeps the model (or_insert — dbt itself errors
    // on duplicates; we degrade to the model).
    for p in process_list {
        for (file_path, raw) in snapshot_blocks(&p.root, &p.file, snapshots) {
            models
                .entry(raw.name.clone())
                .or_insert_with(|| ModelDetails {
                    kind: ModelKind::Snapshot,
                    uri: file_path.clone(),
                    project_name: p.file.name.clone(),
                    description: "Snapshot File".to_string(),
                    ..Default::default()
                });
        }
    }

    props.sweep();
    docs.sweep();
    sql_tags.sweep();
    snapshots.sweep();
    (models, sources, doc_details)
}

/// Every `{% snapshot %}` block under a project's snapshot paths,
/// with the file that holds it (scan-cached per file).
fn snapshot_blocks(
    root: &Path,
    project: &ProjectFile,
    cache: &mut SnapshotsCache,
) -> Vec<(PathBuf, jinja_scan::RawSnapshot)> {
    let mut out = Vec::new();
    for rel in &project.snapshot_paths {
        let dir = root.join(rel);
        if std::fs::read_dir(&dir).is_err() {
            continue;
        }
        for file in util::walk_filepath(&dir, ".sql") {
            let Some(blocks) =
                cache.get_or_parse(&file, |text, _| Some(jinja_scan::extract_snapshots(text)))
            else {
                continue;
            };
            out.extend(blocks.iter().map(|b| (file.clone(), b.clone())));
        }
    }
    out
}

pub(super) struct SchemaModel {
    pub(super) description: String,
    pub(super) access: Option<String>,
    pub(super) group: Option<String>,
    pub(super) contract_enforced: bool,
    pub(super) alias: Option<String>,
    pub(super) schema_uri: PathBuf,
    pub(super) name_pos: Position,
    pub(super) columns: Vec<cogitator_schemas::ColumnEntry>,
    pub(super) tags: Vec<String>,
}

/// Properties YAML files under the model paths (03 §6.2): cached per file;
/// `{{ doc(...) }}` expansion (03 §6.4 / 06 §2.3) applied on retrieval.
fn schema_entries(
    root: &Path,
    project: &ProjectFile,
    docs_map: &HashMap<String, super::docs::DocDetails>,
    props: &mut PropsCache,
) -> (HashMap<String, SchemaModel>, HashMap<String, SourceDetails>) {
    let expand = |description: &str| {
        jinja_scan::replace_doc_refs(description, |name| {
            docs_map.get(name).map(|d| d.content.clone())
        })
    };

    let mut models = HashMap::new();
    let mut sources = HashMap::new();
    for rel in &project.model_paths {
        let dir = root.join(rel);
        if std::fs::read_dir(&dir).is_err() {
            continue;
        }
        for file in util::walk_filepath(&dir, ".yml") {
            let Some(parsed) = props.get_or_parse(&file, |text, path| {
                cogitator_error::ok_or_log(parse_properties(text, path), "properties file")
            }) else {
                continue;
            };
            for model in &parsed.models {
                models.insert(
                    model.name.clone(),
                    SchemaModel {
                        description: expand(&model.description),
                        access: model.access.clone(),
                        group: model.group.clone(),
                        contract_enforced: model.contract_enforced,
                        alias: model.alias.clone(),
                        schema_uri: file.clone(),
                        name_pos: model.name_pos,
                        columns: model.columns.clone(),
                        tags: model.tags.clone(),
                    },
                );
            }
            for source in &parsed.sources {
                sources.insert(source.name.clone(), source_details(source, &file, &expand));
            }
        }
    }
    (models, sources)
}

fn source_details(
    source: &SourceEntry,
    path: &Path,
    expand: &impl Fn(&str) -> String,
) -> SourceDetails {
    let tables = source
        .tables
        .iter()
        .map(|t| {
            (
                t.name.clone(),
                SourceTableDetails {
                    name: t.name.clone(),
                    description: expand(&t.description),
                    uri: path.to_path_buf(),
                    range: Range::zero_width(t.name_pos),
                    identifier: resolve_opt(t.identifier.as_deref()),
                    loader: resolve_opt(t.loader.as_deref()),
                    columns: t.columns.clone(),
                },
            )
        })
        .collect();
    SourceDetails {
        name: source.name.clone(),
        description: expand(&source.description),
        uri: path.to_path_buf(),
        range: Range::zero_width(source.name_pos),
        database: resolve_opt(source.database.as_deref()),
        schema: resolve_opt(source.schema.as_deref()),
        loader: resolve_opt(source.loader.as_deref()),
        tables,
    }
}

/// `env_var()`-resolve an optional raw YAML string from the schemas layer.
fn resolve_opt(raw: Option<&str>) -> Option<String> {
    raw.map(crate::util::resolve_env_vars)
}
