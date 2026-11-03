//! dbt file shapes (specs/rust-port/08 §2).
//!
//! The `generated` module is committed `cargo typify` output from the vendored
//! dbt-labs/dbt-jsonschema files (`schemas/dbt/`); regenerate with the recipe
//! in 08 §2.2. Positions come from a saphyr marker side-table (08 §2.4) built
//! from the same single parse; nothing outside this crate imports `generated`.

pub mod artifacts;
pub mod generated;
pub mod project;
pub mod properties;
pub mod yaml;

pub use project::{parse_dbt_project, ProjectFile};
pub use properties::{
    parse_properties, ColumnEntry, ModelEntry, PropertiesFile, SourceEntry, TableEntry,
};

/// Property-key lists for YAML completion, read from the same processed
/// schemas the typify macro compiles (08 §2.6 v1).
pub mod schema_keys {
    use std::sync::LazyLock;

    static YML_FILES: LazyLock<serde_json::Value> = LazyLock::new(|| {
        serde_json::from_str(include_str!(
            "../../../schemas/dbt/processed/dbt_yml_files.json"
        ))
        .expect("bundled schema is valid json")
    });
    static PROJECT: LazyLock<serde_json::Value> = LazyLock::new(|| {
        serde_json::from_str(include_str!(
            "../../../schemas/dbt/processed/dbt_project.json"
        ))
        .expect("bundled schema is valid json")
    });

    fn keys_at(root: &serde_json::Value, path: &[&str]) -> Vec<String> {
        let mut node = root;
        for step in path {
            node = match node.get(step) {
                Some(next) => next,
                None => return Vec::new(),
            };
        }
        node.get("properties")
            .and_then(|p| p.as_object())
            .map(|o| {
                let mut keys: Vec<String> = o.keys().cloned().collect();
                keys.sort();
                keys
            })
            .unwrap_or_default()
    }

    pub fn properties_root_keys() -> Vec<String> {
        keys_at(&YML_FILES, &[])
    }
    pub fn model_item_keys() -> Vec<String> {
        keys_at(&YML_FILES, &["properties", "models", "items"])
    }
    pub fn source_item_keys() -> Vec<String> {
        keys_at(&YML_FILES, &["properties", "sources", "items"])
    }
    pub fn project_root_keys() -> Vec<String> {
        keys_at(&PROJECT, &[])
    }
}
