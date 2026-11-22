//! Incrementality: could this model, rebuilt in full, be maintained
//! incrementally instead? (specs/ir-pipeline/10)
//!
//! The theory is DBSP (Budiu et al., "DBSP: Incremental Computation on
//! Streams and Its Applications to Databases", SIGMOD Record 53(1), 2024;
//! PVLDB 16(7), 2023). A query Q over a stream of database snapshots has
//! the incremental version Q^Δ = D ∘ Q ∘ I, and the chain rule
//! (Q₁ ∘ Q₂)^Δ = Q₁^Δ ∘ Q₂^Δ lets each operator be incrementalized on its
//! own and the results composed. That is what this module does over the
//! canonical MIR, bottom-up:
//!
//! - linear operators (filter, deterministic projection, UNION ALL) are
//!   their own incremental version, so over an append-only input the
//!   new rows of the output are the query applied to the new input rows:
//!   dbt's `append`;
//! - a join is bilinear, Δa⋈Δb + I(a)⋈Δb + Δa⋈I(b): with one input static
//!   (a dimension) it is linear in the other, with both growing it needs
//!   both histories;
//! - an aggregate is linear per group for SUM and COUNT and needs its
//!   group's history otherwise; dbt keeps that history in the source, so
//!   the groups the new rows touch are recomputed from it and replaced —
//!   `delete+insert` on the group keys, or on a time bucket of the event
//!   column, where only the newest buckets change;
//! - DISTINCT of an append-only input only ever adds rows, merged on all
//!   columns; a window partitioned by a key recomputes touched partitions;
//! - anything else that needs the whole history, or is not a function of
//!   its input (LIMIT, a window over the whole table, EXCEPT, `now()`),
//!   keeps the model a table, and the verdict says which node and why.
//!
//! What cannot be read from the SQL — which inputs only grow, and by what
//! column — comes from `IncrementalInputs` where dbt declares it
//! (`event_time`, a source's `loaded_at_field`, an incremental or snapshot
//! upstream), and is otherwise inferred from a temporal column's name,
//! which the verdict marks as inferred. Unknown parts of the plan give no
//! verdict at all: the analysis never guesses.

mod combine;
mod grouped;
mod inputs;
mod scalar;
mod walk;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::sync::Arc;

use cogitator_ir::mir::{Mir, ProvId};

pub use walk::classify;

/// How an input relation's rows change between dbt runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputGrowth {
    /// Rows are only ever added (a log, events, an append-only source).
    Append,
    /// Rows are updated or deleted (a snapshot, an SCD2 table, CDC).
    Changing,
    /// Nothing is known; inferred from the columns.
    Unknown,
}

/// What dbt says of one input, by relation name: a model's name, or a
/// source table as `source.table`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputFacts {
    pub growth: InputGrowth,
    /// The column that only grows with the rows (`event_time`, a source's
    /// `loaded_at_field`), when declared.
    pub event_time: Option<Arc<str>>,
}

/// Every input dbt declares something about. Sorted, for determinism.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IncrementalInputs {
    pub by_name: BTreeMap<Arc<str>, InputFacts>,
}

/// The input whose new rows drive the increment, and the column that
/// tells new rows apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeltaInput {
    /// `events`, or `raw.orders` for a source table.
    pub relation: String,
    pub column: String,
    /// Declared by dbt, not inferred from a column name.
    pub declared: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    Merge,
    DeleteInsert,
}

impl Strategy {
    pub fn dbt_name(self) -> &'static str {
        match self {
            Strategy::Merge => "merge",
            Strategy::DeleteInsert => "delete+insert",
        }
    }
}

/// Why a model has to stay a table. Each names the plan node it is at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Blocker {
    NonDeterministic(String),
    TemporalFilter(String),
    Limit,
    WindowOverWholeTable,
    AggregateOverWholeTable,
    GroupingSets,
    SetDifference,
    FullJoin,
    OuterJoinOnGrowingSide,
    SubqueryOverGrowingInput,
    ChangingInput(String),
    /// An operator over the output of an aggregate, distinct or window,
    /// whose rows change rather than only grow.
    OverChangingRows(&'static str),
    /// A UNION ALL branch that does not grow would be appended again on
    /// every run.
    StaticUnionBranch,
    NoMonotoneColumn,
    KeysNotInOutput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blocked {
    pub blocker: Blocker,
    pub prov: ProvId,
}

/// What an increment relies on beyond the SQL, said with the verdict.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Note {
    /// Both join inputs grow: the increment also needs the new rows of
    /// `other` matched against the old rows of the driving input.
    Bilinear { other: String },
    /// Inputs assumed not to change for rows already processed.
    AssumesStatic(String),
    /// HAVING can make a group disappear: replace groups, do not merge.
    Having,
    /// SUM/AVG over a float differs by rounding between full and
    /// incremental runs.
    FloatSum,
    /// MIN/MAX are exact only while the input is append-only.
    MinMax,
    /// An aggregate with no incremental form (count distinct, median):
    /// exact here only because touched groups are recomputed whole.
    Holistic(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Linear over an append-only input: `append`, filtered on
    /// `out_column >= max(out_column)` of the model itself.
    Append {
        inputs: Vec<DeltaInput>,
        out_column: String,
    },
    /// Grouped by a bucket of the event column: only the newest buckets
    /// change, replaced with `delete+insert` on `bucket`.
    ByBucket {
        input: DeltaInput,
        bucket: String,
        keys: Vec<String>,
    },
    /// Grouped by keys: the groups the new rows touch are recomputed and
    /// replaced on `keys`. `high_water` is an output column holding the
    /// newest event time of each group, when the model has one.
    ByKey {
        input: DeltaInput,
        keys: Vec<String>,
        strategy: Strategy,
        high_water: Option<String>,
    },
    /// Must stay a table, and why.
    Keep(Vec<Blocked>),
    /// Reads nothing that grows: nothing to increment.
    Static,
    /// Part of the plan is not understood: no verdict.
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Analysis {
    pub verdict: Verdict,
    /// Sorted and deduplicated.
    pub notes: Vec<Note>,
}

impl Analysis {
    /// True when the verdict offers an incremental strategy.
    pub fn incremental(&self) -> bool {
        matches!(
            self.verdict,
            Verdict::Append { .. } | Verdict::ByBucket { .. } | Verdict::ByKey { .. }
        )
    }
}

/// The analysis of the plan's root, with dbt's declarations.
pub fn analyse(mir: &Mir, inputs: Option<&IncrementalInputs>) -> Analysis {
    classify(mir, inputs)
}
