//! dbt project context building (specs/rust-port/03 §1.2): the snapshot
//! types, the build orchestrator, and one submodule per detail-map builder.

pub mod exposures;
mod macros;
mod models;
mod packages;
mod variables;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use cogitator_dialects::Dialect;
use cogitator_lsp::Range;
use cogitator_schemas::project::{apply_defaults, parse_dbt_project, ProjectFile};

use crate::graph::{NodeKind, ProjectGraph};
use crate::util;

use cogitator_parser::jinja_scan::{RawDocsBlock, RawMacro};
use cogitator_schemas::properties::PropertiesFile;

use crate::cache::FileCache;
use crate::graph::EdgeSeed;
use macros::build_macro_details;
use models::build_model_details;
use packages::package_details;
use variables::build_variables;

/// Owns the scan caches across refreshes (05 §6.2 fast path): the refresh
/// task keeps one builder alive, so an unchanged file is never re-parsed.
#[derive(Default)]
pub struct ContextBuilder {
    props: FileCache<PropertiesFile>,
    citizens: FileCache<exposures::CitizenFile>,
    macros: FileCache<Vec<RawMacro>>,
    docs: FileCache<Vec<RawDocsBlock>>,
    edges: FileCache<Vec<EdgeSeed>>,
    /// `{{ config(tags=…) }}` per model file (tag search).
    sql_tags: FileCache<Vec<String>>,
    /// Explicit profiles directory (`--profiles-dir` /
    /// `initializationOptions.profilesDir`); empty → dbt-style search order.
    profiles_dir: String,
    /// Active profile/target overrides (`dbt.setTarget`, `--target`); empty
    /// → the project's profile and its default target.
    profile_override: String,
    target_override: String,
}

impl ContextBuilder {
    pub fn set_profiles_dir(&mut self, dir: impl Into<String>) {
        self.profiles_dir = dir.into();
    }

    pub fn set_profile_override(&mut self, profile: impl Into<String>) {
        self.profile_override = profile.into();
    }

    pub fn set_target_override(&mut self, target: impl Into<String>) {
        self.target_override = target.into();
    }

    /// (hits, misses) across all four artifact caches.
    pub fn cache_stats(&self) -> (u64, u64) {
        [
            self.props.stats(),
            self.macros.stats(),
            self.docs.stats(),
            self.edges.stats(),
            self.sql_tags.stats(),
        ]
        .iter()
        .fold((0, 0), |(h, m), (ch, cm)| (h + ch, m + cm))
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModelDetails {
    pub uri: PathBuf,
    pub project_name: String,
    pub description: String,
    pub schema_uri: Option<PathBuf>,
    pub schema_range: Range,
    /// Documented columns from the properties YAML (write-assist).
    pub columns: Vec<cogitator_schemas::ColumnEntry>,
    /// Tags from the YAML entry and `{{ config(tags=…) }}` (tag search).
    pub tags: Vec<String>,
}

/// One entry of the project-root `selectors.yml` (selector search).
#[derive(Debug, Clone, Default)]
pub struct SelectorDetails {
    pub name: String,
    pub description: String,
    pub uri: PathBuf,
    pub range: Range,
    /// The definition subtree re-serialized as JSON text — dead-model
    /// detection scans it for verbatim model mentions (03 §10).
    pub definition_text: String,
}

#[derive(Debug, Clone, Default)]
pub struct SourceDetails {
    pub name: String,
    pub description: String,
    pub uri: PathBuf,
    pub range: Range,
    /// Relation coordinates from the sources YAML, `env_var()`-resolved.
    pub database: Option<String>,
    pub schema: Option<String>,
    pub loader: Option<String>,
    pub tables: HashMap<String, SourceTableDetails>,
}

#[derive(Debug, Clone, Default)]
pub struct SourceTableDetails {
    pub name: String,
    pub description: String,
    pub uri: PathBuf,
    pub range: Range,
    /// `identifier:` override from the sources YAML, `env_var()`-resolved.
    pub identifier: Option<String>,
    pub loader: Option<String>,
    pub columns: Vec<cogitator_schemas::ColumnEntry>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MacroDetails {
    pub name: String,
    pub project_name: String,
    /// The macro signature text (hover/completion documentation, 03 §5).
    pub description: String,
    /// Structured parameters (06 §2.2 enrichment) for signature help.
    pub params: Vec<cogitator_parser::jinja_scan::MacroParam>,
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
    pub selector_detail_map: HashMap<String, SelectorDetails>,
    /// packages.yml/dependencies.yml declarations, by installable name.
    pub declared_packages: Vec<String>,
    pub exposure_detail_map: HashMap<String, exposures::CitizenDetails>,
    pub metric_detail_map: HashMap<String, exposures::CitizenDetails>,
    pub semantic_model_detail_map: HashMap<String, exposures::CitizenDetails>,
    pub graph: ProjectGraph,
}

pub(super) struct ProjectDetails {
    pub(super) root: PathBuf,
    pub(super) file: ProjectFile,
}

/// One-shot build with a fresh cache (tests / convenience).
pub fn build_context(workspace_root: &str) -> DbtContext {
    ContextBuilder::default().build(workspace_root)
}

impl ContextBuilder {
    /// Build the full context from the LSP client root path (03 §1.2).
    #[tracing::instrument(skip_all, fields(root = %workspace_root))]
    pub fn build(&mut self, workspace_root: &str) -> DbtContext {
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
        let profile = if self.profile_override.is_empty() {
            project.profile.as_str()
        } else {
            self.profile_override.as_str()
        };
        let target = (!self.target_override.is_empty()).then_some(self.target_override.as_str());
        let dialect = util::get_dialect(profile, workspace_root, &self.profiles_dir, target);

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
        let (props_cache, macros_cache, docs_cache, edges_cache, sql_tags_cache) = (
            &mut self.props,
            &mut self.macros,
            &mut self.docs,
            &mut self.edges,
            &mut self.sql_tags,
        );
        // scoped threads lose the span context: hand each builder the
        // parent explicitly so per-builder child spans nest (05 §6.3)
        let parent = tracing::Span::current();
        std::thread::scope(|scope| {
            let models_sources = scope.spawn(|| {
                let _span = tracing::info_span!(parent: &parent, "build_models_sources").entered();
                build_model_details(
                    &root,
                    &project,
                    &process_list,
                    props_cache,
                    docs_cache,
                    sql_tags_cache,
                )
            });
            let macros_handle = scope.spawn(|| {
                let _span = tracing::info_span!(parent: &parent, "build_macros").entered();
                build_macro_details(&process_list, macros_cache)
            });
            let vars_handle = scope.spawn(|| {
                let _span = tracing::info_span!(parent: &parent, "build_variables").entered();
                build_variables(&root, &project)
            });
            let graph_handle = scope.spawn(|| {
                let _span = tracing::info_span!(parent: &parent, "build_graph").entered();
                ProjectGraph::build(&root, &project, &dialect, edges_cache)
            });
            (models, sources) = models_sources.join().unwrap_or_default();
            macros = macros_handle.join().unwrap_or_default();
            variables = vars_handle.join().unwrap_or_default();
            graph = graph_handle.join().unwrap_or_default();
        });

        let selector_detail_map = load_selectors(&root);
        let declared_packages = packages::declared_packages(&root);

        // exposures/metrics/semantic models: a marked-YAML pass over the
        // own project's properties files (03 §10), then into the graph
        let mut citizen_files = Vec::new();
        for rel in &project.model_paths {
            let dir = root.join(rel);
            if std::fs::read_dir(&dir).is_err() {
                continue;
            }
            for file in util::walk_filepath(&dir, ".yml") {
                if let Some(parsed) = self.citizens.get_or_parse(&file, |text, path| {
                    Some(exposures::extract_citizens(text, path))
                }) {
                    citizen_files.push((*parsed).clone());
                }
            }
        }
        self.citizens.sweep();
        let (exposure_detail_map, metric_detail_map, semantic_model_detail_map) =
            exposures::into_maps(citizen_files);
        graph.add_citizens(
            exposure_detail_map
                .values()
                .map(|c| (NodeKind::Exposure, c)),
        );
        graph.add_citizens(metric_detail_map.values().map(|c| (NodeKind::Metric, c)));
        graph.add_citizens(
            semantic_model_detail_map
                .values()
                .map(|c| (NodeKind::SemanticModel, c)),
        );

        DbtContext {
            project_root: Some(root),
            project,
            dialect,
            model_detail_map: models,
            source_detail_map: sources,
            macro_detail_map: macros,
            variable_detail_map: variables,
            selector_detail_map,
            declared_packages,
            exposure_detail_map,
            metric_detail_map,
            semantic_model_detail_map,
            graph,
        }
    }
}

/// `<root>/selectors.yml` → name/description entries with positions
/// (selector search; absent file → empty map).
fn load_selectors(root: &Path) -> HashMap<String, SelectorDetails> {
    use cogitator_schemas::yaml::{as_str, get, items, position_of};
    use saphyr::{LoadableYamlNode, MarkedYaml};

    let path = root.join("selectors.yml");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return HashMap::new();
    };
    let Ok(docs) = MarkedYaml::load_from_str(&text) else {
        return HashMap::new();
    };
    let Some(top) = docs.first() else {
        return HashMap::new();
    };
    let mut out = HashMap::new();
    for entry in get(top, "selectors").into_iter().flat_map(items) {
        let Some(name) = get(entry, "name").and_then(as_str) else {
            continue;
        };
        let position = get(entry, "name").map(position_of).unwrap_or_default();
        out.insert(
            name.to_string(),
            SelectorDetails {
                name: name.to_string(),
                description: get(entry, "description")
                    .and_then(as_str)
                    .unwrap_or_default()
                    .to_string(),
                uri: path.clone(),
                range: Range::zero_width(position),
                definition_text: get(entry, "definition")
                    .map(|d| cogitator_schemas::yaml::to_json(d).to_string())
                    .unwrap_or_default(),
            },
        );
    }
    out
}

/// Parse `<root>/dbt_project.yml` with env_var resolution and defaults
/// (03 §3). Failures degrade to the default/empty project file like Go.
pub(super) fn load_project_file(root: &Path) -> ProjectFile {
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
