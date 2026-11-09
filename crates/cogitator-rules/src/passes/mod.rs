//! The pass manager's contract: phases, budgets, trace and the driver
//! signature (specs/ir-pipeline/04). The driver lands in `feat/ir-passes`.

pub mod budget;
pub mod manager;
pub mod trace;

pub use budget::{Budgets, Exceeded};
pub use manager::{run_passes, PassInput, PassOutput};
pub use trace::{Outcome, Phase, SkipReason, Trace, TraceEntry};
