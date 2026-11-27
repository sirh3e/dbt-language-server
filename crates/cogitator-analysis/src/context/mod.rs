//! dbt project context building (specs/rust-port/03 §1.2): the snapshot
//! types, the build orchestrator, and one submodule per detail-map builder.

pub mod docs;
pub mod env;
pub mod exposures;
pub mod functions;
mod macros;
mod models;
mod packages;
pub mod persist;
pub mod types;
mod var_reads;
mod variables;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use cogitator_dialects::Dialect;
use cogitator_lsp::Range;
use cogitator_schemas::project::{apply_defaults, parse_dbt_project, ProjectFile};

use crate::graph::{NodeKind, ProjectGraph};
use crate::util;

use cogitator_parser::jinja_scan::{RawDocsBlock, RawMacro, RawSnapshot};
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
    /// The `{% materialization %}` names of each macro file.
    materializations: FileCache<Vec<String>>,
    snapshots: FileCache<Vec<RawSnapshot>>,
    docs: FileCache<Vec<RawDocsBlock>>,
    /// `functions:` entries per properties file under the function paths.
    functions: functions::FunctionsCache,
    edges: FileCache<Vec<EdgeSeed>>,
    /// `{{ config(tags=…) }}` per model file (tag search).
    sql_tags: FileCache<Vec<String>>,
    /// The var names each file reads (unused-var diagnostics).
    var_reads: FileCache<Vec<String>>,
    /// Explicit profiles directory (`--profiles-dir` /
    /// `initializationOptions.profilesDir`); empty → dbt-style search order.
    profiles_dir: String,
    /// Active profile/target overrides (`dbt.setTarget`, `--target`); empty
    /// → the project's profile and its default target.
    profile_override: String,
    target_override: String,
    /// `--vars` equivalents (cogitator.yml / initializationOptions).
    var_overrides: Vec<VarOverride>,
}

impl ContextBuilder {
    pub fn set_profiles_dir(&mut self, dir: impl Into<String>) {
        self.profiles_dir = dir.into();
    }

    pub fn set_var_overrides(&mut self, overrides: Vec<VarOverride>) {
        self.var_overrides = overrides;
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
            self.snapshots.stats(),
            self.docs.stats(),
            self.functions.stats(),
            self.edges.stats(),
            self.sql_tags.stats(),
        ]
        .iter()
        .fold((0, 0), |(h, m), (ch, cm)| (h + ch, m + cm))
    }
}

pub use var_reads::{var_names_read, VarReads};

pub use types::{
    MacroDetails, ModelDetails, ModelKind, PackageVars, SelectorDetails, SourceDetails,
    SourceTableDetails, VarOverride, VarSource, VariableDetails,
};

#[derive(Debug, Default)]
pub struct DbtContext {
    pub project_root: Option<PathBuf>,
    pub project: ProjectFile,
    pub dialect: Dialect,
    pub model_detail_map: HashMap<String, ModelDetails>,
    pub source_detail_map: HashMap<String, SourceDetails>,
    /// package name → macro name → details (03 §5).
    pub macro_detail_map: HashMap<String, HashMap<String, MacroDetails>>,
    /// The root project's vars (03 §4), overrides included.
    pub variable_detail_map: HashMap<String, VariableDetails>,
    /// Each installed package's vars as its own files see them.
    pub package_variables: Vec<PackageVars>,
    /// Project name → the var names its files read (`None` before a scan).
    pub var_reads: Option<var_reads::VarReads>,
    /// What `env_var()` sees: process environment, `.env`, profiles uses.
    pub env: env::EnvVars,
    pub selector_detail_map: HashMap<String, SelectorDetails>,
    /// `<root>/selectors.yml` read as set expressions (selector engine).
    pub selectors: crate::selectors::yaml::SelectorsFile,
    /// packages.yml/dependencies.yml declarations, by installable name.
    pub declared_packages: Vec<String>,
    pub exposure_detail_map: HashMap<String, exposures::CitizenDetails>,
    pub metric_detail_map: HashMap<String, exposures::CitizenDetails>,
    pub semantic_model_detail_map: HashMap<String, exposures::CitizenDetails>,
    /// `{% docs %}` blocks by name (03 §6.4): own project wins on collision.
    pub doc_detail_map: HashMap<String, docs::DocDetails>,
    /// User-defined functions (`function('…')`, dbt 1.11+) by name: own
    /// project wins on collision.
    pub function_detail_map: HashMap<String, functions::FunctionDetails>,
    /// Every function body file of the projects (`is_function_body`).
    pub function_bodies: std::collections::HashSet<PathBuf>,
    pub graph: ProjectGraph,
    /// The names of the `{% materialization %}` blocks of the project and
    /// its packages (the built-in ones are not among them).
    pub materializations: std::collections::BTreeSet<String>,
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
        let mut doc_details = HashMap::new();
        let mut macros = HashMap::new();
        let mut materializations = std::collections::BTreeSet::new();
        let mut variables = HashMap::new();
        let mut package_variables = Vec::new();
        let mut graph = ProjectGraph::default();
        let (props_cache, macros_cache, docs_cache, edges_cache, sql_tags_cache) = (
            &mut self.props,
            &mut self.macros,
            &mut self.docs,
            &mut self.edges,
            &mut self.sql_tags,
        );
        let snapshots_cache = &mut self.snapshots;
        let materializations_cache = &mut self.materializations;
        let var_reads_cache = &mut self.var_reads;
        let mut var_reads = var_reads::VarReads::new();
        let var_overrides = &self.var_overrides;
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
                    snapshots_cache,
                )
            });
            let macros_handle = scope.spawn(|| {
                let _span = tracing::info_span!(parent: &parent, "build_macros").entered();
                build_macro_details(&process_list, macros_cache, materializations_cache)
            });
            let vars_handle = scope.spawn(|| {
                let _span = tracing::info_span!(parent: &parent, "build_variables").entered();
                build_variables(&root, &project, &process_list[1..], var_overrides)
            });
            let reads_handle = scope.spawn(|| {
                let _span = tracing::info_span!(parent: &parent, "build_var_reads").entered();
                var_reads::build_var_reads(&process_list, &dialect, var_reads_cache)
            });
            let graph_handle = scope.spawn(|| {
                let _span = tracing::info_span!(parent: &parent, "build_graph").entered();
                ProjectGraph::build(&root, &project, &dialect, edges_cache)
            });
            (models, sources, doc_details) = models_sources.join().unwrap_or_default();
            (macros, materializations) = macros_handle.join().unwrap_or_default();
            (variables, package_variables) = vars_handle.join().unwrap_or_default();
            graph = graph_handle.join().unwrap_or_default();
            var_reads = reads_handle.join().unwrap_or_default();
        });

        // Refs to seeds and snapshots created empty-uri graph nodes during
        // the model-file scan; give them their real files so dangling-ref
        // detection and navigation treat them as present (05 §5).
        graph.fill_model_uris(|name| models.get(name).map(|d| d.uri.clone()));

        let functions = functions::build_function_details(&process_list, &mut self.functions);
        let (selector_detail_map, selectors) = load_selectors(&root);
        let declared_packages = packages::declared_packages(&root);
        let env = env::load_env(&root, &self.profiles_dir, workspace_root);

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
            package_variables,
            var_reads: Some(var_reads),
            env,
            selector_detail_map,
            selectors,
            declared_packages,
            exposure_detail_map,
            metric_detail_map,
            semantic_model_detail_map,
            doc_detail_map: doc_details,
            function_detail_map: functions.details,
            function_bodies: functions.bodies,
            graph,
            materializations,
        }
    }
}

/// `<root>/selectors.yml` → name/description entries with positions
/// (selector search) and the definitions as set expressions (absent
/// file → empty).
fn load_selectors(
    root: &Path,
) -> (
    HashMap<String, SelectorDetails>,
    crate::selectors::yaml::SelectorsFile,
) {
    use cogitator_schemas::yaml::{get, items, to_json};
    use saphyr::{LoadableYamlNode, MarkedYaml};

    let path = root.join("selectors.yml");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Default::default();
    };
    let file = crate::selectors::yaml::parse(&text);
    // the definitions' JSON text, for dead-model detection's verbatim scan
    let mut json = HashMap::new();
    if let Some(top) = MarkedYaml::load_from_str(&text)
        .ok()
        .and_then(|d| d.into_iter().next())
    {
        for entry in get(&top, "selectors").into_iter().flat_map(items) {
            let name = get(entry, "name").map(to_json);
            if let (Some(serde_json::Value::String(name)), Some(def)) =
                (name, get(entry, "definition"))
            {
                json.insert(name, to_json(def).to_string());
            }
        }
    }
    let out = file
        .definitions
        .iter()
        .map(|d| {
            let details = SelectorDetails {
                name: d.name.clone(),
                description: d.description.clone(),
                uri: path.clone(),
                range: Range::zero_width(d.range.start),
                definition_text: json.remove(&d.name).unwrap_or_default(),
            };
            (d.name.clone(), details)
        })
        .collect();
    (out, file)
}

/// [`load_project_file`] for other modules: a package's dbt_project.yml.
pub(crate) fn load_project_file_at(root: &Path) -> ProjectFile {
    load_project_file(root)
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
    // key ranges from the raw text: a resolved env_var() shifts columns
    if resolved != text {
        if let Some((top, nested)) = cogitator_schemas::project::var_ranges(&text, &path) {
            file.var_ranges = top;
            file.nested_var_ranges = nested;
        }
    }
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

/// A path as a `file://` URI spells it, back as a path: no Windows
/// verbatim prefix, `/` separators, the drive letter upper case.
fn as_uri_spells_it(path: &Path) -> PathBuf {
    let uri = cogitator_lsp::fs_path_to_uri(path);
    let plain = cogitator_lsp::uri_to_fs_path(&uri).unwrap_or_else(|| path.display().to_string());
    let mut chars = plain.chars();
    match (chars.next(), chars.next()) {
        (Some(drive), Some(':')) if drive.is_ascii_alphabetic() => {
            PathBuf::from(format!("{}{}", drive.to_ascii_uppercase(), &plain[1..]))
        }
        _ => PathBuf::from(plain),
    }
}

impl DbtContext {
    /// Is the file at `uri` a user-defined function's body (the project's
    /// or a package's, an overload's too)? Its SQL is an expression, no
    /// statement, so the model checks do not apply to it. A new body file
    /// counts once the next refresh has seen it.
    ///
    /// The paths are compared as a URI spells them: the CLI canonicalizes
    /// its project root, which on Windows is a verbatim path
    /// (`\\?\C:\p\functions\f.sql`), while a file's URI never is
    /// (`fs_path_to_uri` strips the prefix), so the plain `Path`s differ.
    pub fn is_function_body(&self, uri: &str) -> bool {
        let Some(path) = cogitator_lsp::uri_to_fs_path(uri) else {
            return false;
        };
        let wanted = as_uri_spells_it(Path::new(&path));
        self.function_bodies
            .iter()
            .any(|body| as_uri_spells_it(body) == wanted)
    }

    /// The installed package a file belongs to (its files see the
    /// package's vars), by path under the package's root.
    /// Compared as a URI spells the paths, as in [`Self::is_function_body`].
    pub fn package_of_uri(&self, uri: &str) -> Option<&PackageVars> {
        let path = cogitator_lsp::uri_to_fs_path(uri).unwrap_or_else(|| uri.to_string());
        let path = as_uri_spells_it(Path::new(&path));
        self.package_variables
            .iter()
            .map(|p| (p, as_uri_spells_it(&p.root)))
            .filter(|(_, root)| path.starts_with(root))
            .max_by_key(|(_, root)| root.as_os_str().len())
            .map(|(p, _)| p)
    }

    /// The vars a file sees (03 §4): its package's, else the root project's.
    pub fn vars_for(&self, uri: &str) -> &HashMap<String, VariableDetails> {
        self.package_of_uri(uri)
            .map(|p| &p.vars)
            .unwrap_or(&self.variable_detail_map)
    }

    /// The var a `var('name')` in `uri` resolves to.
    pub fn var_for(&self, uri: &str, name: &str) -> Option<&VariableDetails> {
        self.vars_for(uri).get(name)
    }
}

#[cfg(test)]
mod uri_path_tests {
    use super::as_uri_spells_it;
    use std::path::Path;

    #[test]
    fn a_verbatim_path_is_its_uris_path() {
        // what `canonicalize` gives on Windows, against a URI's path
        let verbatim = as_uri_spells_it(Path::new(r"\\?\C:\p\functions\f.sql"));
        let plain = as_uri_spells_it(Path::new("c:/p/functions/f.sql"));
        assert_eq!(verbatim, plain);
        assert_eq!(
            as_uri_spells_it(Path::new("/p/functions/f.sql")),
            Path::new("/p/functions/f.sql")
        );
    }
}
