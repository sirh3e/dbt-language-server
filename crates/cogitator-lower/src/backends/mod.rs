//! One `DialectBackend` per dialect. Empty until `feat/ir-lowering`.

pub mod bigquery;
pub mod duckdb;
pub mod generic;
pub mod postgres;
pub mod snowflake;

use cogitator_ir::lir::Backend;

use crate::DialectBackend;

/// The backend for `backend`, once its module implements one.
pub fn for_backend(_backend: Backend) -> Option<&'static dyn DialectBackend> {
    None
}
