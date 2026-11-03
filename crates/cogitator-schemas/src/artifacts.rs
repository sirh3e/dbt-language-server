//! JSON artifact types (08 §4): the SANCTIONED re-export of the
//! generated artifact modules. Unlike the YAML schemas these carry no
//! positions, so no side-table discipline applies — the generated
//! shapes ARE the adapter input; cogitator-analysis maps them into its
//! lean public indexes immediately and nothing else touches them.

pub mod run_results {
    pub use crate::generated::run_results::*;
}
pub mod sources {
    pub use crate::generated::sources::*;
}
pub mod catalog {
    pub use crate::generated::catalog::*;
}
pub mod manifest {
    pub use crate::generated::manifest::*;
}

/// Expected $id per artifact — for version TRACING, never rejection
/// (03 §10: read, trace, never reject).
pub const RUN_RESULTS_SCHEMA_V6: &str = "https://schemas.getdbt.com/dbt/run-results/v6.json";
pub const SOURCES_SCHEMA_V3: &str = "https://schemas.getdbt.com/dbt/sources/v3.json";
pub const CATALOG_SCHEMA_V1: &str = "https://schemas.getdbt.com/dbt/catalog/v1.json";
pub const MANIFEST_SCHEMA_V12: &str = "https://schemas.getdbt.com/dbt/manifest/v12.json";
