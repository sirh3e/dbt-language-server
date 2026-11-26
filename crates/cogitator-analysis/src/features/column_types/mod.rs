//! Output-column types of a model, with their provenance, and the checks
//! built on them (FEATURE_REQUESTS: type checking).
//!
//! The types are read off the project schema job's cached inference
//! (`plan::schemas::SchemaSnapshot::outputs`, filled once per job from the
//! model's MIR), so nothing here plans a model again. Each column carries
//! its lattice type, the dialect's spelling when the type is pinned
//! exactly, where the type came from ([`Provenance`]) and how far it can
//! be trusted ([`Basis`]): a type is `Proven` only when every upstream
//! column it rests on is typed by SQL inference, the catalog or an
//! enforced contract. Unknown stays unknown.
//!
//! Public API for other features (and the breaking-change preview):
//! [`column_types`] for one model, [`compare`] / [`render`] for types.
//!
//! Checks: [`type_mismatch_diagnostics`] (`column-type-mismatch`, YAML
//! side), [`unit_test_value_diagnostics`] (`unit-test-value-type`),
//! [`fill_data_types_action`], the `inlay.types` hints
//! [`type_inlay_hints`], and the hovers [`sql_column_hover`] /
//! [`yaml_column_hover`].

mod basis;
mod checks;
mod compare;
mod hover;
mod inlay;
mod provenance;
mod unit_values;
mod yaml_columns;

#[cfg(test)]
pub(crate) mod tests;
#[cfg(test)]
mod tests_checks;
#[cfg(test)]
mod tests_compare;
#[cfg(test)]
mod tests_inlay;

pub use basis::{column_types, Basis, ColumnType, Feed, ModelColumnTypes, Origin};
pub use checks::{fill_data_types_action, type_mismatch_diagnostics, yaml_column_hover, MISMATCH};
pub use compare::{compare, describe, render, Verdict};
pub use hover::{sql_column_hover, type_line};
pub use inlay::type_inlay_hints;
pub use provenance::{is_stated, raw_columns, Provenance, RawColumn, Relation, UpstreamColumn};
pub use unit_values::{unit_test_value_diagnostics, VALUE_TYPE};
