//! dbt file shapes (specs/rust-port/08 §2).
//!
//! The `generated` module is committed `cargo typify` output from the vendored
//! dbt-labs/dbt-jsonschema files (`schemas/dbt/`); regenerate with the recipe
//! in 08 §2.2. Positions come from a saphyr marker side-table (08 §2.4) built
//! from the same single parse; nothing outside this crate imports `generated`.

pub mod generated;
pub mod project;
pub mod properties;
pub mod yaml;

pub use project::{parse_dbt_project, ProjectFile};
pub use properties::{parse_properties, ModelEntry, PropertiesFile, SourceEntry, TableEntry};
