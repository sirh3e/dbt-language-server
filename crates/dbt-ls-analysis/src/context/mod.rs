//! dbt project context building (specs/rust-port/03 §1.2): the snapshot
//! types, the build orchestrator, and one submodule per detail-map builder.

mod macros;
mod models;
mod packages;
mod variables;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use dbt_ls_dialects::Dialect;
use dbt_ls_lsp::Range;
use dbt_ls_schemas::project::{apply_defaults, parse_dbt_project, ProjectFile};

use crate::graph::ProjectGraph;
use crate::util;

use macros::build_macro_details;
use models::build_model_details;
use packages::package_details;
use variables::build_variables;

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

pub(super) struct ProjectDetails {
    pub(super) root: PathBuf,
    pub(super) file: ProjectFile,
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
