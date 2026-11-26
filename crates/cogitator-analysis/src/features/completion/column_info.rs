//! One fact per column the editor offers or hovers: its type and where
//! that type was read, its description and where that came from. The
//! completion list, the item's documentation and the column hover all
//! render it through [`markdown`], so they say the same thing.
//!
//! Type, in order: the YAML `data_type`, `target/catalog.json`, the schema
//! job's inferred type (with its basis). Description: the YAML's (its
//! `{{ doc() }}` already resolved by the context); for an undocumented
//! column one inherited down a direct pass-through (a model column read
//! straight from one upstream column; a CTE column named like a column
//! of exactly one relation the document reads).

use cogitator_schemas::ColumnEntry;

use super::snippets::ModelHints;
use super::DbtContext;
use crate::features::column_types::{self, Basis, Provenance, Relation};

/// Which relation a column belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ColumnOf {
    Model(String),
    Source {
        source: String,
        table: String,
    },
    /// A CTE of the document: no YAML of its own.
    Cte(String),
}

impl ColumnOf {
    pub(crate) fn label(&self) -> String {
        match self {
            ColumnOf::Model(m) => m.clone(),
            ColumnOf::Source { source, table } => format!("{source}.{table}"),
            ColumnOf::Cte(c) => format!("{c} (CTE)"),
        }
    }
}

/// Where a column's type was read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypeFrom {
    Yaml,
    Catalog,
    Inferred(Basis),
}

impl TypeFrom {
    fn words(self) -> &'static str {
        match self {
            TypeFrom::Yaml => "YAML data_type",
            TypeFrom::Catalog => "from catalog",
            TypeFrom::Inferred(Basis::Proven) => "inferred",
            TypeFrom::Inferred(_) => "inferred, not proven",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ColumnFact {
    pub(crate) of: ColumnOf,
    pub(crate) column: String,
    pub(crate) ty: Option<(String, TypeFrom)>,
    pub(crate) description: String,
    /// The column a description (or a CTE column's type) was inherited from.
    pub(crate) inherited: Option<String>,
}

fn yaml_entry<'a>(ctx: &'a DbtContext, of: &ColumnOf, column: &str) -> Option<&'a ColumnEntry> {
    let columns = match of {
        ColumnOf::Model(m) => &ctx.model_detail_map.get(m)?.columns,
        ColumnOf::Source { source, table } => {
            &ctx.source_detail_map
                .get(source)?
                .tables
                .get(table)?
                .columns
        }
        ColumnOf::Cte(_) => return None,
    };
    columns.iter().find(|c| c.name.eq_ignore_ascii_case(column))
}

fn catalog_type(hints: &ModelHints, of: &ColumnOf, column: &str) -> Option<String> {
    let catalog = hints.catalog.as_deref()?;
    let table = match of {
        ColumnOf::Model(m) => catalog.models.get(m)?,
        ColumnOf::Source { source, table } => {
            catalog.sources.get(&(source.clone(), table.clone()))?
        }
        ColumnOf::Cte(_) => return None,
    };
    table
        .columns
        .iter()
        .find(|c| c.name.eq_ignore_ascii_case(column))
        .map(|c| c.data_type.to_ascii_lowercase())
}

/// The fact of `column` of `of`. `reads` are the relations the document
/// reads, for a CTE column's inheritance.
pub(crate) fn column_fact(
    ctx: &DbtContext,
    hints: &ModelHints,
    of: &ColumnOf,
    column: &str,
    reads: &[ColumnOf],
) -> ColumnFact {
    if let ColumnOf::Cte(_) = of {
        let found: Vec<ColumnFact> = reads
            .iter()
            .filter(|r| !matches!(r, ColumnOf::Cte(_)))
            .map(|r| column_fact(ctx, hints, r, column, &[]))
            .filter(|f| f.ty.is_some() || !f.description.is_empty())
            .collect();
        return match found.as_slice() {
            [one] => ColumnFact {
                of: of.clone(),
                column: column.to_string(),
                ty: one.ty.clone(),
                description: one.description.clone(),
                inherited: Some(format!("{}.{}", one.of.label(), one.column)),
            },
            _ => ColumnFact {
                of: of.clone(),
                column: column.to_string(),
                ty: None,
                description: String::new(),
                inherited: None,
            },
        };
    }
    let entry = yaml_entry(ctx, of, column);
    let inferred = match of {
        ColumnOf::Model(m) => hints
            .snapshot
            .as_deref()
            .and_then(|s| column_types::column_types(ctx, s, m))
            .and_then(|t| t.column(column).cloned()),
        _ => None,
    };
    let ty = entry
        .and_then(|e| e.data_type.clone())
        .map(|t| (t, TypeFrom::Yaml))
        .or_else(|| catalog_type(hints, of, column).map(|t| (t, TypeFrom::Catalog)))
        .or_else(|| {
            let c = inferred.as_ref()?;
            column_types::is_stated(&c.sql_type).then(|| {
                (
                    c.data_type.clone().unwrap_or_else(|| c.description.clone()),
                    TypeFrom::Inferred(c.basis),
                )
            })
        });
    let mut description = entry.map(|e| e.description.clone()).unwrap_or_default();
    let mut inherited = None;
    if description.is_empty() {
        if let Some(c) = inferred.as_ref() {
            if let Provenance::Upstream { from } = &c.provenance {
                let up = match &from.relation {
                    Relation::Model { name } => Some(ColumnOf::Model(name.clone())),
                    Relation::Source { source, table } => Some(ColumnOf::Source {
                        source: source.clone(),
                        table: table.clone(),
                    }),
                    _ => None,
                };
                if let Some(up) = up {
                    if let Some(e) = yaml_entry(ctx, &up, &from.column) {
                        if !e.description.is_empty() {
                            description = e.description.clone();
                            inherited = Some(format!("{}.{}", up.label(), e.name));
                        }
                    }
                }
            }
        }
    }
    ColumnFact {
        of: of.clone(),
        column: entry.map_or_else(|| column.to_string(), |e| e.name.clone()),
        ty,
        description,
        inherited,
    }
}

/// The documentation of a column, the same in completion and hover:
///
/// ```text
/// `customer_id` · `bigint` (YAML data_type)
///
/// Column of stg_customers
///
/// The customer's id.
///
/// Description from stg_orders.customer_id
/// ```
pub(crate) fn markdown(fact: &ColumnFact) -> String {
    let mut text = format!("`{}`", fact.column);
    if let Some((ty, from)) = &fact.ty {
        text.push_str(&format!(" · `{ty}` ({})", from.words()));
    }
    text.push_str(&format!("\n\nColumn of {}", fact.of.label()));
    if !fact.description.is_empty() {
        text.push_str("\n\n");
        text.push_str(&fact.description);
    }
    if let Some(from) = &fact.inherited {
        text.push_str(&format!("\n\n*Description from {from}*"));
    }
    text
}
