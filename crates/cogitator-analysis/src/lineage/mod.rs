//! Column lineage (FEATURE_REQUESTS): which upstream columns feed each
//! output column of a model. Pure over the document text; exposed through
//! the `dbt.columnLineage` workspace command.
//!
//! Pipeline: the front end builds a bound HIR of the live template over an
//! empty schema environment, and [`from_hir`] walks it, mapping every
//! projected column back to models, sources or CTE outputs. Templates with
//! Jinja control blocks decline (the 07 §3.2 confidence rule: blanked
//! branches would fabricate lineage).

use serde::Serialize;

mod from_hir;

pub use from_hir::column_lineage;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum SourceRef {
    /// Another dbt model (`{{ ref('…') }}`).
    Model { name: String, column: String },
    /// A dbt source table (`{{ source('a', 'b') }}`).
    Source {
        source: String,
        table: String,
        column: String,
    },
    /// A relation the file does not define (raw table, outer CTE, …).
    Unknown { name: String, column: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct ColumnLineage {
    pub name: String,
    pub sources: Vec<SourceRef>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelLineage {
    pub columns: Vec<ColumnLineage>,
}

#[cfg(test)]
mod corpus_tests;
#[cfg(test)]
mod gap_tests;
