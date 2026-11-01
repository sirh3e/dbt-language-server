//! Committed `cargo typify` output — DO NOT EDIT (specs/rust-port/08 §2.2).
//!
//! Regeneration (after updating `schemas/dbt/`): the dbt_yml_files schema is
//! preprocessed first (case-duplicate enum values deduped, `if`/`then`/`else`
//! validation refinements stripped — typify 0.8 supports neither); the other
//! schemas generate as-is. See schemas/dbt/README.

#[allow(clippy::all, dead_code, unused)]
pub mod dbt_project;
#[allow(clippy::all, dead_code, unused)]
pub mod dbt_yml_files;
#[allow(clippy::all, dead_code, unused)]
pub mod packages;
#[allow(clippy::all, dead_code, unused)]
pub mod selectors;
