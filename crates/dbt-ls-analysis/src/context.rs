//! dbt project context building (specs/rust-port/03 §1.2, §3–§8).
//!
//! `build_context` is the Go `refreshDbtContext`: it walks the project (and
//! installed packages) and produces an immutable snapshot the server
//! publishes as `Arc<DbtContext>` (05 §6.2).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use dbt_ls_dialects::Dialect;
use dbt_ls_lsp::{Position, Range};
use dbt_ls_parser::jinja_scan;
use dbt_ls_schemas::project::{apply_defaults, parse_dbt_project, ProjectFile};
use dbt_ls_schemas::properties::parse_properties;

use crate::graph::ProjectGraph;
use crate::util;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModelDetails {
    pub uri: PathBuf,
    pub project_name: String,
    pub description: String,
    pub schema_uri: Option<PathBuf>,
    pub schema_range: Range,
}

#[derive(Debug, Clone, Default)]
pub struct SourceDetails {
    pub name: String,
    pub description: String,
    pub uri: PathBuf,
    pub range: Range,
    pub tables: HashMap<String, SourceTableDetails>,
}

#[derive(Debug, Clone, Default)]
pub struct SourceTableDetails {
    pub name: String,
    pub description: String,
    pub uri: PathBuf,
    pub range: Range,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MacroDetails {
    pub name: String,
    pub project_name: String,
    /// The macro signature text (hover/completion documentation, 03 §5).
    pub description: String,
    pub uri: PathBuf,
    pub range: Range,
}

#[derive(Debug, Clone, Default)]
pub struct VariableDetails {
    pub name: String,
    pub value: serde_json::Value,
    pub uri: PathBuf,
    pub range: Range,
}

#[derive(Debug, Default)]
pub struct DbtContext {
    pub project_root: Option<PathBuf>,
    pub project: ProjectFile,
    pub dialect: Dialect,
    pub model_detail_map: HashMap<String, ModelDetails>,
    pub source_detail_map: HashMap<String, SourceDetails>,
    /// package name → macro name → details (03 §5).
    pub macro_detail_map: HashMap<String, HashMap<String, MacroDetails>>,
    pub variable_detail_map: HashMap<String, VariableDetails>,
    pub graph: ProjectGraph,
}

struct ProjectDetails {
    root: PathBuf,
    file: ProjectFile,
}

/// Build the full context from the LSP client root path (03 §1.2).
pub fn build_context(workspace_root: &str) -> DbtContext {
    let start = if workspace_root.is_empty() {
        std::env::current_dir().unwrap_or_default()
    } else {
        PathBuf::from(workspace_root)
    };
    let Some(root) = util::get_project_root("dbt_project.yml", &start) else {
        tracing::debug!(?start, "dbt_project.yml not found");
        return DbtContext::default();
    };

    let project = load_project_file(&root);
    let dialect = util::get_dialect(&project.profile, workspace_root);

    let own = ProjectDetails {
        root: root.clone(),
        file: project.clone(),
    };
    let packages = package_details(&root, &project);
    let mut process_list = vec![own];
    process_list.extend(packages);

    // The three parallel builders of 03 §1.2 plus the graph (05 §5), on
    // scoped threads — CPU + fs work stays off the async runtime (05 §6.2).
    let mut models = HashMap::new();
    let mut sources = HashMap::new();
    let mut macros = HashMap::new();
    let mut variables = HashMap::new();
    let mut graph = ProjectGraph::default();
    std::thread::scope(|scope| {
        let models_sources = scope.spawn(|| build_model_details(&root, &project, &process_list));
        let macros_handle = scope.spawn(|| build_macro_details(&process_list));
        let vars_handle = scope.spawn(|| build_variables(&root, &project));
        let graph_handle = scope.spawn(|| ProjectGraph::build(&root, &project, &dialect));
        (models, sources) = models_sources.join().unwrap_or_default();
        macros = macros_handle.join().unwrap_or_default();
        variables = vars_handle.join().unwrap_or_default();
        graph = graph_handle.join().unwrap_or_default();
    });

    DbtContext {
        project_root: Some(root),
        project,
        dialect,
        model_detail_map: models,
        source_detail_map: sources,
        macro_detail_map: macros,
        variable_detail_map: variables,
        graph,
    }
}

/// Parse `<root>/dbt_project.yml` with env_var resolution and defaults
/// (03 §3). Failures degrade to the default/empty project file like Go.
fn load_project_file(root: &Path) -> ProjectFile {
    let path = root.join("dbt_project.yml");
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(err) => {
            tracing::debug!(?path, %err, "cannot read dbt_project.yml");
            return ProjectFile::default();
        }
    };
    let resolved = util::resolve_env_vars(&text);
    let mut file = match parse_dbt_project(&resolved, &path) {
        Ok(f) => f,
        Err(err) => {
            tracing::debug!(?path, %err, "cannot parse dbt_project.yml");
            ProjectFile::default()
        }
    };
    let dirs = list_dirs(root);
    apply_defaults(&mut file, &dirs);
    file
}

fn list_dirs(root: &Path) -> Vec<String> {
    std::fs::read_dir(root)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.path().is_dir())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// Installed packages (03 §8): direct subdirectories of the install path,
/// each with its own parsed+defaulted dbt_project.yml.
fn package_details(root: &Path, project: &ProjectFile) -> Vec<ProjectDetails> {
    if project.packages_install_path.is_empty() {
        return Vec::new();
    }
    let install = root.join(&project.packages_install_path);
    std::fs::read_dir(install)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.path().is_dir())
                .map(|e| {
                    let pkg_root = e.path();
                    let file = load_project_file(&pkg_root);
                    ProjectDetails {
                        root: pkg_root,
                        file,
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Models + seeds + sources (03 §6).
fn build_model_details(
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
    let mut models = HashMap::new();
    let mut sources = HashMap::new();
    let expand = |description: &str| {
        jinja_scan::replace_doc_refs(description, |name| docs_map.get(name).cloned())
    };

    for path in &project.model_paths {
        let dir = root.join(path);
        if std::fs::read_dir(&dir).is_err() {
            continue;
        }
        for file in util::walk_filepath(&dir, ".yml") {
            let Ok(text) = std::fs::read_to_string(&file) else {
                continue;
            };
            let parsed = match parse_properties(&text, &file) {
                Ok(p) => p,
                Err(err) => {
                    tracing::debug!(?file, %err, "skipping properties file");
                    continue;
                }
            };
            for model in parsed.models {
                models.insert(
                    model.name.clone(),
                    SchemaModel {
                        description: expand(&model.description),
                        alias: model.alias,
                        schema_uri: file.clone(),
                        name_pos: model.name_pos,
                    },
                );
            }
            for source in parsed.sources {
                let tables = source
                    .tables
                    .into_iter()
                    .map(|t| {
                        (
                            t.name.clone(),
                            SourceTableDetails {
                                name: t.name,
                                description: expand(&t.description),
                                uri: file.clone(),
                                range: Range::zero_width(t.name_pos),
                            },
                        )
                    })
                    .collect();
                sources.insert(
                    source.name.clone(),
                    SourceDetails {
                        name: source.name,
                        description: expand(&source.description),
                        uri: file.clone(),
                        range: Range::zero_width(source.name_pos),
                        tables,
                    },
                );
            }
        }
    }

    (models, sources)
}

/// `{% docs %}` blocks from the docs paths (03 §6.4; Q4 fixed: paths are
/// joined to the project root).
fn build_docs_map(root: &Path, project: &ProjectFile) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for path in &project.docs_paths {
        let dir = root.join(path);
        if std::fs::read_dir(&dir).is_err() {
            continue;
        }
        for file in util::walk_filepath(&dir, ".md") {
            let Ok(text) = std::fs::read_to_string(&file) else {
                continue;
            };
            for block in jinja_scan::extract_docs_blocks(&text) {
                map.insert(block.name, block.content);
            }
        }
    }
    map
}

/// Macros across own project + packages (03 §5 / 06 §2.2).
fn build_macro_details(
    process_list: &[ProjectDetails],
) -> HashMap<String, HashMap<String, MacroDetails>> {
    let mut map: HashMap<String, HashMap<String, MacroDetails>> = HashMap::new();
    for p in process_list {
        for path in &p.file.macro_paths {
            let dir = p.root.join(path);
            if std::fs::read_dir(&dir).is_err() {
                continue;
            }
            for file in util::walk_filepath(&dir, ".sql") {
                let Ok(text) = std::fs::read_to_string(&file) else {
                    continue;
                };
                for raw in jinja_scan::extract_macros(&text) {
                    map.entry(p.file.name.clone()).or_default().insert(
                        raw.name.clone(),
                        MacroDetails {
                            name: raw.name,
                            project_name: p.file.name.clone(),
                            description: raw.signature,
                            uri: file.clone(),
                            range: Range {
                                start: Position {
                                    line: raw.start.0,
                                    character: raw.start.1,
                                },
                                end: Position {
                                    line: raw.end.0,
                                    character: raw.end.1,
                                },
                            },
                        },
                    );
                }
            }
        }
    }
    map
}

/// Project variables (03 §4).
fn build_variables(root: &Path, project: &ProjectFile) -> HashMap<String, VariableDetails> {
    let mut vars = HashMap::new();
    let project_uri = root.join("dbt_project.yml");

    for (key, value) in &project.vars {
        if key == &project.name {
            continue;
        }
        let pos = project.var_positions.get(key).copied().unwrap_or_default();
        vars.insert(
            key.clone(),
            VariableDetails {
                name: key.clone(),
                value: value.clone(),
                uri: project_uri.clone(),
                range: Range::zero_width(pos),
            },
        );
    }

    // Project-scoped vars override, skipping null values (03 §4).
    if let Some(serde_json::Value::Object(scoped)) = project.vars.get(&project.name) {
        let positions = project.nested_var_positions.get(&project.name);
        for (key, value) in scoped {
            if value.is_null() {
                continue;
            }
            let pos = positions
                .and_then(|m| m.get(key))
                .copied()
                .unwrap_or_default();
            vars.insert(
                key.clone(),
                VariableDetails {
                    name: key.clone(),
                    value: value.clone(),
                    uri: project_uri.clone(),
                    range: Range::zero_width(pos),
                },
            );
        }
    }

    vars
}
