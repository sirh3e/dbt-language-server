//! The per-entity detail records the context exposes: what hover,
//! definition and the graph browsers read for each model, source,
//! macro, variable and selector.

use std::collections::HashMap;
use std::path::PathBuf;

use cogitator_lsp::Range;

/// What produced a `model_detail_map` entry — everything `ref()`-able.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ModelKind {
    #[default]
    Model,
    Seed,
    Snapshot,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModelDetails {
    pub kind: ModelKind,
    pub uri: PathBuf,
    pub project_name: String,
    pub description: String,
    pub schema_uri: Option<PathBuf>,
    pub schema_range: Range,
    /// Documented columns from the properties YAML (write-assist).
    pub columns: Vec<cogitator_schemas::ColumnEntry>,
    /// Tags from the YAML entry and `{{ config(tags=…) }}` (tag search).
    pub tags: Vec<String>,
    /// Governance (03 §10 addendum): `access:` of the YAML entry.
    pub access: Option<String>,
    /// Governance group of the YAML entry.
    pub group: Option<String>,
    /// `contract.enforced: true` on the YAML entry.
    pub contract_enforced: bool,
    /// `event_time` of the YAML entry: the column that only grows.
    pub event_time: Option<String>,
    /// Column sets the YAML entry declares unique (model-level
    /// `constraints` and `unique_combination_of_columns` tests), each
    /// lower-cased and sorted.
    pub unique_keys: Vec<Vec<String>>,
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
    /// `event_time` or `loaded_at_field` of the table, else of its
    /// source: the column that only grows with the rows.
    pub event_time: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MacroDetails {
    pub name: String,
    pub project_name: String,
    /// The macro signature text (hover/completion documentation, 03 §5).
    pub description: String,
    /// Structured parameters (06 §2.2 enrichment) for signature help.
    pub params: Vec<cogitator_parser::jinja_scan::MacroParam>,
    /// Body facts: varargs/kwargs reads and the dispatch it makes.
    pub body: cogitator_parser::macro_body::MacroBody,
    pub uri: PathBuf,
    pub range: Range,
}

#[derive(Debug, Clone, Default)]
pub struct VariableDetails {
    pub name: String,
    pub value: serde_json::Value,
    /// The file that decided the value (dbt_project.yml, a package's
    /// dbt_project.yml, cogitator.yml); empty for an override from
    /// initializationOptions.
    pub uri: PathBuf,
    /// The entry's KEY range in that file.
    pub range: Range,
    pub source: VarSource,
}

/// Where a var's value came from, lowest precedence first (dbt's
/// variable precedence: --vars, root project scoped, root project
/// global, package scoped, package global).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum VarSource {
    /// A package's own dbt_project.yml (global or scoped to itself).
    Package(String),
    /// The root project's top-level `vars:`.
    #[default]
    Project,
    /// The root project's `vars: {<scope>: …}` block for this scope.
    Scoped(String),
    /// cogitator.yml `vars:` or initializationOptions.vars (like `--vars`).
    Override,
}

/// One override (`--vars` equivalent) with where it was written.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VarOverride {
    pub name: String,
    pub value: serde_json::Value,
    /// cogitator.yml, or empty for initializationOptions.
    pub uri: PathBuf,
    pub range: Range,
}

/// An installed package's vars as its own models and macros see them.
#[derive(Debug, Clone, Default)]
pub struct PackageVars {
    /// The package's project name (its dbt_project.yml `name`).
    pub name: String,
    pub root: PathBuf,
    pub vars: std::collections::HashMap<String, VariableDetails>,
}
