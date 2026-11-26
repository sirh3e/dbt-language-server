//! Column lineage (FEATURE_REQUESTS): which upstream columns feed each
//! output column of a model. Pure over the document text; exposed through
//! the `dbt.columnLineage` workspace command.
//!
//! Pipeline: the front end builds a bound HIR of the live template over an
//! empty schema environment, and [`from_hir`] walks it, mapping every
//! projected column back to models, sources or CTE outputs. Templates with
//! Jinja control blocks decline (the 07 §3.2 confidence rule: blanked
//! branches would fabricate lineage). [`typed_column_lineage`] answers the
//! same question with the project's schemas and fresh compiled SQL.

use serde::Serialize;

mod from_hir;
mod typed;

pub use from_hir::column_lineage;
pub(crate) use from_hir::{column_lineage_expanded, has_control_block, template_input};
pub use typed::{needs_compiled, typed_column_lineage};

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
    /// The column passes one upstream column through unchanged: a bare
    /// reference under the same name at every hop (CTEs, derived tables,
    /// `select *`), no expression, rename, set operation or replacement.
    /// What documentation may be inherited along (`features::yaml_sync`);
    /// not on the wire.
    #[serde(skip)]
    pub direct: bool,
}

/// Where an answer that was not read off the live template came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LineageOrigin {
    /// `target/compiled`, known fresh when this answer was computed.
    Compiled,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelLineage {
    pub columns: Vec<ColumnLineage>,
    /// Absent on the template route.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<LineageOrigin>,
}

impl ModelLineage {
    /// `" (from compiled SQL)"` when that is where this came from, for
    /// messages built from it.
    pub fn origin_note(&self) -> &'static str {
        match self.origin {
            Some(LineageOrigin::Compiled) => " (from compiled SQL)",
            None => "",
        }
    }
}

#[cfg(test)]
mod corpus_tests;
#[cfg(test)]
mod direct_tests;
#[cfg(test)]
mod gap_tests;
#[cfg(test)]
mod typed_tests;
