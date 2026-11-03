//! Column lineage (FEATURE_REQUESTS): which upstream columns feed each
//! output column of a model. Pure over the document text; exposed through
//! the `dbt.columnLineage` workspace command.
//!
//! Pipeline: the already-parsed token stream locates the Jinja constructs →
//! the SQL is rebuilt with *named* placeholders (`__dbt_rel_N__` ↔ ref/source
//! targets) → sqlparser parses the masked statement → CTE-aware resolution
//! maps every projected column back to models, sources or CTE outputs.
//! Templates with Jinja control blocks decline (the 07 §3.2 confidence rule:
//! blanked branches would fabricate lineage).

use std::collections::HashMap;

use cogitator_dialects::Dialect;
use cogitator_parser::ParsedDocument;
use serde::Serialize;
use sqlparser::ast::{SetExpr, Statement};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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

/// Relation targets a FROM/JOIN item can resolve to.
#[derive(Debug, Clone)]
enum Relation {
    Model(String),
    Source(String, String),
    Cte(String),
    Unknown(String),
}

/// Entry point: lineage for one model document. `None` when the template
/// declines (control blocks) or the masked SQL does not parse.
pub fn column_lineage(doc: &ParsedDocument, dialect: &Dialect) -> Option<ModelLineage> {
    let (masked, placeholders) = mask_for_lineage(doc)?;
    let statements = sqlparser::parser::Parser::parse_sql(
        cogitator_parser::backend::sql_dialect(dialect).as_ref(),
        &masked,
    )
    .ok()?;
    let query = statements.iter().find_map(|s| match s {
        Statement::Query(q) => Some(q),
        _ => None,
    })?;

    // CTEs resolve in declaration order and may reference earlier ones.
    let mut ctes: HashMap<String, Vec<ColumnLineage>> = HashMap::new();
    if let Some(with) = &query.with {
        for cte in &with.cte_tables {
            let name = cte.alias.name.value.clone();
            if let SetExpr::Select(select) = cte.query.body.as_ref() {
                let columns = select_lineage(select, &placeholders, &ctes);
                ctes.insert(name, columns);
            }
        }
    }

    let SetExpr::Select(select) = query.body.as_ref() else {
        return None; // set operations: take nothing rather than guess (v1)
    };
    Some(ModelLineage {
        columns: select_lineage(select, &placeholders, &ctes),
    })
}

// ---------------------------------------------------------------------------
// Masking
// ---------------------------------------------------------------------------

mod mask;
mod resolve;

use mask::mask_for_lineage;
use resolve::select_lineage;
