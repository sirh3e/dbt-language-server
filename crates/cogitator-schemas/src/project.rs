//! `dbt_project.yml` adapter (specs/rust-port/03 §3, 08 §2.3).
//!
//! Deserializes into the typify-generated `DbtProject` type and extracts the
//! fields the server uses, plus the vars position side-table. Defaults
//! (03 §3.3) are applied by `apply_defaults` from a caller-supplied directory
//! listing so this module stays pure.

use std::collections::HashMap;
use std::path::Path;

use cogitator_error::Result;
use cogitator_lsp::Range;

use crate::generated::dbt_project::DbtProject;
use crate::yaml;

#[derive(Debug, Clone, Default)]
pub struct ProjectFile {
    pub name: String,
    pub profile: String,
    pub model_paths: Vec<String>,
    pub seed_paths: Vec<String>,
    pub snapshot_paths: Vec<String>,
    pub test_paths: Vec<String>,
    pub analysis_paths: Vec<String>,
    pub macro_paths: Vec<String>,
    pub packages_install_path: String,
    pub docs_paths: Vec<String>,
    /// `docs-paths` was explicitly set in the YAML (03 §3.3: append rule only
    /// applies when it was not).
    pub docs_paths_explicit: bool,
    /// Raw `vars` mapping (top level).
    pub vars: serde_json::Map<String, serde_json::Value>,
    /// Range of each top-level `vars` entry's KEY (definition target).
    pub var_ranges: HashMap<String, Range>,
    /// Range side-table for nested var maps: top-level key → (inner key →
    /// inner key range). Used for project- and package-scoped vars (03 §4).
    pub nested_var_ranges: HashMap<String, HashMap<String, Range>>,
    /// `dispatch:` entries (dbt's macro dispatch search order).
    pub dispatch: Vec<DispatchConfig>,
}

/// One `dispatch:` entry: where `adapter.dispatch(…, macro_namespace)`
/// looks, in order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DispatchConfig {
    pub macro_namespace: String,
    pub search_order: Vec<String>,
}

/// `dispatch:` read leniently from the project JSON: an entry without a
/// namespace or with a non-list search order is dropped.
fn dispatch_of(value: &serde_json::Value) -> Vec<DispatchConfig> {
    let Some(entries) = value.get("dispatch").and_then(|d| d.as_array()) else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|e| {
            let macro_namespace = e.get("macro_namespace")?.as_str()?.to_string();
            let search_order = e
                .get("search_order")?
                .as_array()?
                .iter()
                .filter_map(|p| p.as_str().map(str::to_string))
                .collect();
            Some(DispatchConfig {
                macro_namespace,
                search_order,
            })
        })
        .collect()
}

/// Parse `dbt_project.yml` text (already env_var-resolved, 03 §3.2).
///
/// Degrades like Go (03 §3): any parse failure yields an `Err`; callers log
/// and continue with `ProjectFile::default()`.
pub fn parse_dbt_project(text: &str, path: &Path) -> Result<ProjectFile> {
    let marked = yaml::load_marked(text, path)?;
    let mut value = yaml::to_json(&marked);
    let dispatch = dispatch_of(&value);
    // The server consumes only these keys (03 §3). Resource-config subtrees
    // (`models:`, `seeds:`, …) vary widely in the wild and the generated
    // unions are strict, so everything else is pruned before deserialization
    // — a rejected config block must not cost us the whole file (05 §2).
    const USED_KEYS: &[&str] = &[
        "name",
        "profile",
        "model-paths",
        "seed-paths",
        "snapshot-paths",
        "test-paths",
        "analysis-paths",
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
        snapshot_paths: paths_of(project.snapshot_paths.as_ref()),
        test_paths: paths_of(project.test_paths.as_ref()),
        analysis_paths: paths_of(project.analysis_paths.as_ref()),
        macro_paths: paths_of(project.macro_paths.as_ref()),
        packages_install_path: project.packages_install_path.clone().unwrap_or_default(),
        docs_paths: paths_of(project.docs_paths.as_ref()),
        docs_paths_explicit: project.docs_paths.is_some(),
        vars: project.vars.clone(),
        dispatch,
        ..Default::default()
    };

    (file.var_ranges, file.nested_var_ranges) = ranges_in(&marked);

    Ok(file)
}

/// Key ranges of the `vars:` entries (top level, and one level of
/// nesting for scope blocks).
pub type VarRanges = (
    HashMap<String, Range>,
    HashMap<String, HashMap<String, Range>>,
);

fn ranges_in(marked: &saphyr::MarkedYaml<'_>) -> VarRanges {
    let mut top = HashMap::new();
    let mut nested = HashMap::new();
    if let Some(vars_node) = yaml::get(marked, "vars") {
        for (k, v) in yaml::entries(vars_node) {
            let Some(key) = yaml::as_str(k) else { continue };
            top.insert(key.to_string(), yaml::range_of(k));
            let inner: HashMap<String, Range> = yaml::entries(v)
                .filter_map(|(nk, _)| yaml::as_str(nk).map(|s| (s.to_string(), yaml::range_of(nk))))
                .collect();
            if !inner.is_empty() {
                nested.insert(key.to_string(), inner);
            }
        }
    }
    (top, nested)
}

/// The `vars:` key ranges of a `dbt_project.yml` text — read from the RAW
/// text, so an `env_var()` resolved elsewhere on a line cannot shift the
/// columns (`None` when the raw text does not parse as YAML).
pub fn var_ranges(text: &str, path: &Path) -> Option<VarRanges> {
    yaml::load_marked(text, path).ok().map(|m| ranges_in(&m))
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
    if file.snapshot_paths.is_empty() && has("snapshots") {
        file.snapshot_paths = vec!["snapshots".into()];
    }
    if file.test_paths.is_empty() && has("tests") {
        file.test_paths = vec!["tests".into()];
    }
    if file.analysis_paths.is_empty() {
        for dir in ["analyses", "analysis"] {
            if has(dir) {
                file.analysis_paths = vec![dir.into()];
                break;
            }
        }
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
