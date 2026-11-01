//! `dbt_project.yml` adapter (specs/rust-port/03 §3, 08 §2.3).
//!
//! Deserializes into the typify-generated `DbtProject` type and extracts the
//! fields the server uses, plus the vars position side-table. Defaults
//! (03 §3.3) are applied by `apply_defaults` from a caller-supplied directory
//! listing so this module stays pure.

use std::collections::HashMap;
use std::path::Path;

use dbt_ls_error::Result;
use dbt_ls_lsp::Position;

use crate::generated::dbt_project::DbtProject;
use crate::yaml;

#[derive(Debug, Clone, Default)]
pub struct ProjectFile {
    pub name: String,
    pub profile: String,
    pub model_paths: Vec<String>,
    pub seed_paths: Vec<String>,
    pub macro_paths: Vec<String>,
    pub packages_install_path: String,
    pub docs_paths: Vec<String>,
    /// `docs-paths` was explicitly set in the YAML (03 §3.3: append rule only
    /// applies when it was not).
    pub docs_paths_explicit: bool,
    /// Raw `vars` mapping (top level).
    pub vars: serde_json::Map<String, serde_json::Value>,
    /// Position of each top-level `vars` entry's value node.
    pub var_positions: HashMap<String, Position>,
    /// Position side-table for nested var maps: top-level key → (inner key →
    /// inner value position). Used for project-scoped vars (03 §4).
    pub nested_var_positions: HashMap<String, HashMap<String, Position>>,
}

/// Parse `dbt_project.yml` text (already env_var-resolved, 03 §3.2).
///
/// Degrades like Go (03 §3): any parse failure yields an `Err`; callers log
/// and continue with `ProjectFile::default()`.
pub fn parse_dbt_project(text: &str, path: &Path) -> Result<ProjectFile> {
    let marked = yaml::load_marked(text, path)?;
    let mut value = yaml::to_json(&marked);
    // The server consumes only these keys (03 §3). Resource-config subtrees
    // (`models:`, `seeds:`, …) vary widely in the wild and the generated
    // unions are strict, so everything else is pruned before deserialization
    // — a rejected config block must not cost us the whole file (05 §2).
    const USED_KEYS: &[&str] = &[
        "name",
        "profile",
        "model-paths",
        "seed-paths",
        "macro-paths",
        "packages-install-path",
        "docs-paths",
        "vars",
    ];
    if let serde_json::Value::Object(map) = &mut value {
        map.retain(|k, _| USED_KEYS.contains(&k.as_str()));
    }
    let project: DbtProject = serde_json::from_value(value)?;

    let mut file = ProjectFile {
        name: project.name.clone(),
        profile: project.profile.clone().unwrap_or_default(),
        model_paths: paths_of(project.model_paths.as_ref()),
        seed_paths: paths_of(project.seed_paths.as_ref()),
        macro_paths: paths_of(project.macro_paths.as_ref()),
        packages_install_path: project.packages_install_path.clone().unwrap_or_default(),
        docs_paths: paths_of(project.docs_paths.as_ref()),
        docs_paths_explicit: project.docs_paths.is_some(),
        vars: project.vars.clone(),
        ..Default::default()
    };

    if let Some(vars_node) = yaml::get(&marked, "vars") {
        for (k, v) in yaml::entries(vars_node) {
            let Some(key) = yaml::as_str(k) else { continue };
            file.var_positions
                .insert(key.to_string(), yaml::position_of(v));
            let nested: HashMap<String, Position> = yaml::entries(v)
                .filter_map(|(nk, nv)| {
                    yaml::as_str(nk).map(|s| (s.to_string(), yaml::position_of(nv)))
                })
                .collect();
            if !nested.is_empty() {
                file.nested_var_positions.insert(key.to_string(), nested);
            }
        }
    }

    Ok(file)
}

fn paths_of(arr: Option<&crate::generated::dbt_project::ArrayOfStrings>) -> Vec<String> {
    arr.map(|a| a.iter().cloned().collect()).unwrap_or_default()
}

/// Post-parse defaults (03 §3.3). `available_dirs` lists the names of the
/// directories directly under the project root.
pub fn apply_defaults(file: &mut ProjectFile, available_dirs: &[String]) {
    let has = |name: &str| available_dirs.iter().any(|d| d == name);
    if file.model_paths.is_empty() && has("models") {
        file.model_paths = vec!["models".into()];
    }
    if file.seed_paths.is_empty() && has("seeds") {
        file.seed_paths = vec!["seeds".into()];
    }
    if file.macro_paths.is_empty() && has("macros") {
        file.macro_paths = vec!["macros".into()];
    }
    if file.packages_install_path.is_empty() && has("dbt_packages") {
        file.packages_install_path = "dbt_packages".into();
    }
    if !file.docs_paths_explicit && file.docs_paths.is_empty() {
        if has("docs") {
            file.docs_paths.push("docs".into());
        }
        file.docs_paths.extend(file.model_paths.iter().cloned());
        file.docs_paths.extend(file.macro_paths.iter().cloned());
    }
}
