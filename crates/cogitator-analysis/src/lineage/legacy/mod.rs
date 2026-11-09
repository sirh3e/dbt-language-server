//! The token/AST column lineage this crate shipped before HIR, kept only so
//! the parity test can diff it against `from_hir`. Unchanged apart from
//! moving here and the name of its entry point; removed once parity has
//! held for a release (tranche 2 cleanup).

use std::collections::HashMap;

use cogitator_dialects::Dialect;
use cogitator_parser::ParsedDocument;
use sqlparser::ast::{SetExpr, Statement};

use super::{ColumnLineage, ModelLineage};

mod mask;
mod resolve;

use mask::mask_for_lineage;
use resolve::select_lineage;

/// Relation targets a FROM/JOIN item can resolve to.
#[derive(Debug, Clone)]
enum Relation {
    Model(String),
    Source(String, String),
    Cte(String),
    Unknown(String),
}

/// Lineage for one model document as the legacy engine computed it.
pub(crate) fn legacy_column_lineage(
    doc: &ParsedDocument,
    dialect: &Dialect,
) -> Option<ModelLineage> {
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
