//! dbt file shapes (specs/rust-port/08 §2), on the shared `dbt-schemas`
//! crate.
//!
//! The vendored dbt schemas, their typify types, the marked-YAML helpers,
//! the artifact types and the schema walks live in `dbt-schemas`
//! (https://github.com/sirh3e/dbt-schemas), which other dbt tools read too.
//! This crate keeps what is cogitator's: the project and properties
//! adapters with their LSP position side-tables (08 §2.4), on
//! `cogitator-error` and `cogitator-lsp`. Nothing outside this crate
//! imports `generated`.

pub use dbt_schemas::{artifacts, generated, processed, schema_keys, schema_walk};

pub mod project;
pub mod properties;
pub mod yaml;

pub use project::{parse_dbt_project, DispatchConfig, ProjectFile};
pub use properties::{
    parse_properties, ColumnEntry, ModelEntry, PropertiesFile, SourceEntry, TableEntry,
};
