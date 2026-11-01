//! Models, seeds, sources and `{% docs %}` expansion (03 §6):
//! filesystem collection at the boundary, pure folds below it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use cogitator_lsp::{Position, Range};
use cogitator_parser::jinja_scan;
use cogitator_schemas::project::ProjectFile;
use cogitator_schemas::properties::parse_properties;

use crate::fs_scan::{read_sources, SourceFile};
use crate::util;

use super::{ModelDetails, ProjectDetails, SourceDetails, SourceTableDetails};

/// Models + seeds + sources (03 §6).
pub(super) fn build_model_details(
    root: &Path,
    project: &ProjectFile,
    process_list: &[ProjectDetails],
) -> (
    HashMap<String, ModelDetails>,
    HashMap<String, SourceDetails>,
) {
    let mut models = HashMap::new();
    let mut sources = HashMap::new();

    for p in process_list {
        let path_map = util::create_file_name_map(&[".sql", ".py"], &p.root, &p.file.model_paths);
        let docs_map = build_docs_map(&p.root, &p.file);
        let (schema_models, project_sources) = parse_yaml_models(&p.root, &p.file, &docs_map);

        sources.extend(project_sources);

        for (stem, file_path) in path_map {
            let schema = schema_models.get(&stem);
            let key = schema
                .and_then(|m| m.alias.clone())
                .filter(|a| !a.is_empty())
                .unwrap_or_else(|| stem.clone());
            models.insert(
                key,
                ModelDetails {
                    uri: file_path,
                    project_name: p.file.name.clone(),
                    description: schema.map(|m| m.description.clone()).unwrap_or_default(),
                    schema_uri: schema.map(|m| m.schema_uri.clone()),
                    schema_range: schema
                        .map(|m| Range::zero_width(m.name_pos))
                        .unwrap_or_default(),
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
                uri: file_path,
                project_name: project.name.clone(),
                description: "Seed File".to_string(),
                schema_uri: None,
                schema_range: Range::default(),
            },
        );
    }

    (models, sources)
}

struct SchemaModel {
    description: String,
    alias: Option<String>,
    schema_uri: PathBuf,
    name_pos: Position,
}

/// Properties YAML files under the model paths (03 §6.2), with `{% docs %}`
/// expansion applied to every description (03 §6.4 / 06 §2.3).
fn parse_yaml_models(
    root: &Path,
    project: &ProjectFile,
    docs_map: &HashMap<String, String>,
) -> (HashMap<String, SchemaModel>, HashMap<String, SourceDetails>) {
    let files = read_sources(root, &project.model_paths, ".yml");
    models_sources_from_yaml(&files, docs_map)
}

/// Pure over the collected files: parse each properties YAML, expand
/// `{{ doc(...) }}` references, and fold into the two maps (later files win
/// per name, like Go's walk order).
fn models_sources_from_yaml(
    files: &[SourceFile],
    docs_map: &HashMap<String, String>,
) -> (HashMap<String, SchemaModel>, HashMap<String, SourceDetails>) {
    let expand = |description: &str| {
        jinja_scan::replace_doc_refs(description, |name| docs_map.get(name).cloned())
    };

    files
        .iter()
        .filter_map(|f| {
            cogitator_error::ok_or_log(parse_properties(&f.text, &f.path), "properties file")
                .map(|parsed| (f, parsed))
        })
        .fold(
            (HashMap::new(), HashMap::new()),
            |(mut models, mut sources), (f, parsed)| {
                for model in parsed.models {
                    models.insert(
                        model.name.clone(),
                        SchemaModel {
                            description: expand(&model.description),
                            alias: model.alias,
                            schema_uri: f.path.clone(),
                            name_pos: model.name_pos,
                        },
                    );
                }
                for source in parsed.sources {
                    sources.insert(
                        source.name.clone(),
                        source_details(source, &f.path, &expand),
                    );
                }
                (models, sources)
            },
        )
}

fn source_details(
    source: cogitator_schemas::properties::SourceEntry,
    path: &Path,
    expand: &impl Fn(&str) -> String,
) -> SourceDetails {
    let tables = source
        .tables
        .into_iter()
        .map(|t| {
            (
                t.name.clone(),
                SourceTableDetails {
                    name: t.name,
                    description: expand(&t.description),
                    uri: path.to_path_buf(),
                    range: Range::zero_width(t.name_pos),
                },
            )
        })
        .collect();
    SourceDetails {
        name: source.name,
        description: expand(&source.description),
        uri: path.to_path_buf(),
        range: Range::zero_width(source.name_pos),
        tables,
    }
}

/// `{% docs %}` blocks from the docs paths (03 §6.4; Q4 fixed: paths are
/// joined to the project root).
fn build_docs_map(root: &Path, project: &ProjectFile) -> HashMap<String, String> {
    docs_map_from_sources(&read_sources(root, &project.docs_paths, ".md"))
}

/// Pure: `{% docs %}` blocks across files, last block per name wins.
fn docs_map_from_sources(files: &[SourceFile]) -> HashMap<String, String> {
    files
        .iter()
        .flat_map(|f| jinja_scan::extract_docs_blocks(&f.text))
        .map(|block| (block.name, block.content))
        .collect()
}
