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
