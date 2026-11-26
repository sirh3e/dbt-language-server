//! The columns of a FROM item: a model's from its properties YAML (else
//! its column lineage, read through the shell's `read`), a source table's
//! from its YAML, a CTE's or a derived table's from its select list, `*`
//! and `t.*` expanded through the relations it reads.

use std::path::Path;

use sqlparser::ast::{Expr, Query, Select, SelectItem, SelectItemQualifiedWildcardKind, SetExpr};

use super::super::super::column_info::ColumnOf;
use super::super::super::snippets::ModelHints;
use super::super::{cte_columns, RelationTarget};
use super::{walk, Relation, Scoped, SqlScope};
use crate::context::DbtContext;
use crate::Document;

/// How deep CTEs reading CTEs are followed (a recursive CTE stops here).
const MAX_DEPTH: usize = 8;

/// A column a relation in scope has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Column {
    pub(crate) name: String,
    /// What the column is of: `customers`, `jaffle.raw_orders`,
    /// `recent (CTE)`.
    pub(crate) relation: String,
    pub(crate) data_type: Option<String>,
    pub(crate) description: String,
    /// The model or source table the column is one of, for its facts
    /// (`column_info`); none for a CTE's or a derived table's, whose type
    /// and description are what they pass through.
    pub(crate) of: Option<ColumnOf>,
}

/// The read-only things a column lookup needs.
pub(crate) struct Lookup<'a> {
    pub(crate) ctx: &'a DbtContext,
    pub(crate) doc: &'a Document,
    pub(crate) read: &'a dyn Fn(&Path) -> Option<String>,
    /// The shell's caches: a model's inferred columns its YAML lacks.
    pub(crate) hints: &'a ModelHints,
}

impl SqlScope<'_> {
    /// The columns of one FROM item of the cursor's SELECT.
    pub(crate) fn columns(&self, lookup: &Lookup<'_>, scoped: &Scoped) -> Vec<Column> {
        self.relation_columns(lookup, &scoped.relation, 0)
    }

    /// The columns of the CTE `name`, through its parsed select list.
    pub(crate) fn cte_columns(&self, lookup: &Lookup<'_>, name: &str) -> Vec<Column> {
        self.target_columns(lookup, &RelationTarget::Cte(name.to_string()), 0)
    }

    fn relation_columns(
        &self,
        lookup: &Lookup<'_>,
        relation: &Relation,
        depth: usize,
    ) -> Vec<Column> {
        match relation {
            Relation::Target(target) => self.target_columns(lookup, target, depth),
            Relation::Derived(query) => {
                relabel(self.query_columns(lookup, query, depth + 1), "subquery")
            }
        }
    }

    /// A CTE through its parsed select list, anything else as the token
    /// scope would see it.
    fn target_columns(
        &self,
        lookup: &Lookup<'_>,
        target: &RelationTarget,
        depth: usize,
    ) -> Vec<Column> {
        let RelationTarget::Cte(name) = target else {
            return target_columns(lookup, target);
        };
        match self.ctes.get(&name.to_lowercase()) {
            Some(query) if depth < MAX_DEPTH => relabel(
                self.query_columns(lookup, query, depth + 1),
                &format!("{name} (CTE)"),
            ),
            Some(_) => Vec::new(),
            None => target_columns(lookup, target),
        }
    }

    /// A query's output columns, in select-list order.
    fn query_columns(&self, lookup: &Lookup<'_>, query: &Query, depth: usize) -> Vec<Column> {
        if depth > MAX_DEPTH {
            return Vec::new();
        }
        match first_select(&query.body) {
            Some(select) => self.select_columns(lookup, select, depth),
            None => Vec::new(),
        }
    }

    fn select_columns(&self, lookup: &Lookup<'_>, select: &Select, depth: usize) -> Vec<Column> {
        let scoped = self.namer.scoped(&walk::items(select));
        let of = |s: &Scoped| self.relation_columns(lookup, &s.relation, depth);
        // what a bare or qualified name passes through from
        let upstream = |qualifier: Option<&str>, name: &str| -> Option<Column> {
            scoped
                .iter()
                .filter(|s| qualifier.is_none_or(|q| s.qualifier.eq_ignore_ascii_case(q)))
                .find_map(|s| {
                    of(s)
                        .into_iter()
                        .find(|c| c.name.eq_ignore_ascii_case(name))
                })
        };
        let named = |name: &str, from: Option<Column>| Column {
            name: name.to_string(),
            relation: String::new(),
            data_type: from.as_ref().and_then(|c| c.data_type.clone()),
            description: from.map(|c| c.description).unwrap_or_default(),
            of: None,
        };
        let mut out: Vec<Column> = Vec::new();
        for item in &select.projection {
            match item {
                SelectItem::UnnamedExpr(expr) => {
                    if let Some((qualifier, name)) = column_ref(expr) {
                        out.push(named(name, upstream(qualifier, name)));
                    }
                }
                SelectItem::ExprWithAlias { expr, alias } => {
                    let from = column_ref(expr).and_then(|(q, n)| upstream(q, n));
                    out.push(named(&alias.value, from));
                }
                SelectItem::ExprWithAliases { aliases, .. } => {
                    out.extend(aliases.iter().map(|a| named(&a.value, None)));
                }
                SelectItem::Wildcard(_) => out.extend(scoped.iter().flat_map(of)),
                SelectItem::QualifiedWildcard(
                    SelectItemQualifiedWildcardKind::ObjectName(name),
                    _,
                ) => {
                    let qualifier = name
                        .0
                        .last()
                        .and_then(|p| p.as_ident())
                        .map(|i| i.value.as_str());
                    out.extend(
                        scoped
                            .iter()
                            .filter(|s| {
                                qualifier.is_some_and(|q| s.qualifier.eq_ignore_ascii_case(q))
                            })
                            .flat_map(of),
                    );
                }
                SelectItem::QualifiedWildcard(..) => {}
            }
        }
        let mut seen = std::collections::HashSet::new();
        out.retain(|c| seen.insert(c.name.to_lowercase()));
        out
    }
}

/// `col` or `q.col` as (qualifier, name).
fn column_ref(expr: &Expr) -> Option<(Option<&str>, &str)> {
    match expr {
        Expr::Identifier(ident) => Some((None, ident.value.as_str())),
        Expr::CompoundIdentifier(parts) => {
            let name = parts.last()?;
            let qualifier = parts.len().checked_sub(2).map(|i| parts[i].value.as_str());
            Some((qualifier, name.value.as_str()))
        }
        _ => None,
    }
}

fn first_select(set: &SetExpr) -> Option<&Select> {
    match set {
        SetExpr::Select(select) => Some(select),
        SetExpr::Query(query) => first_select(&query.body),
        SetExpr::SetOperation { left, .. } => first_select(left),
        _ => None,
    }
}

fn relabel(columns: Vec<Column>, relation: &str) -> Vec<Column> {
    columns
        .into_iter()
        .map(|c| Column {
            relation: relation.to_string(),
            // a CTE's or derived table's column: what it passes through
            // is in its type and description, not in its own facts
            of: None,
            ..c
        })
        .collect()
}

/// A model's documented columns; without any, what its lineage says it
/// produces (an unexpanded `*` is left out, never guessed).
fn model_columns(lookup: &Lookup<'_>, name: &str) -> Vec<Column> {
    let Some(model) = lookup.ctx.model_detail_map.get(name) else {
        return Vec::new();
    };
    let column = |column: String, data_type, description| Column {
        name: column,
        relation: name.to_string(),
        data_type,
        description,
        of: Some(ColumnOf::Model(name.to_string())),
    };
    // what the schema job infers the model gives, for the columns its
    // YAML does not list
    let inferred = lookup
        .hints
        .snapshot
        .as_deref()
        .and_then(|s| crate::features::column_types::column_types(lookup.ctx, s, name));
    let with_inferred = |mut columns: Vec<Column>| {
        for c in inferred.iter().flat_map(|t| &t.columns) {
            if !columns.iter().any(|k| k.name.eq_ignore_ascii_case(&c.name)) {
                let ty = crate::features::column_types::is_stated(&c.sql_type)
                    .then(|| c.data_type.clone().unwrap_or_else(|| c.description.clone()));
                columns.push(column(c.name.clone(), ty, String::new()));
            }
        }
        columns
    };
    if !model.columns.is_empty() {
        return with_inferred(
            model
                .columns
                .iter()
                .map(|c| column(c.name.clone(), c.data_type.clone(), c.description.clone()))
                .collect(),
        );
    }
    let lineage = (lookup.read)(&model.uri)
        .and_then(|text| {
            cogitator_parser::pipeline::parse_document(&text, &lookup.ctx.dialect).ok()
        })
        .and_then(|parsed| crate::lineage::column_lineage(&parsed, &lookup.ctx.dialect))
        .map(|lineage| {
            lineage
                .columns
                .into_iter()
                .filter(|c| !c.name.contains('*'))
                .map(|c| column(c.name, None, String::new()))
                .collect()
        })
        .unwrap_or_default();
    with_inferred(lineage)
}

/// The columns of a relation the token scope found (no parse to read a
/// CTE's select list from: its columns are the token scan's).
pub(crate) fn target_columns(lookup: &Lookup<'_>, target: &RelationTarget) -> Vec<Column> {
    match target {
        RelationTarget::Cte(name) => cte_columns(lookup.doc, name)
            .into_iter()
            .map(|column| Column {
                name: column,
                relation: format!("{name} (CTE)"),
                data_type: None,
                description: String::new(),
                of: None,
            })
            .collect(),
        RelationTarget::Model(name) => model_columns(lookup, name),
        RelationTarget::SourceTable { source, table } => source_columns(lookup, source, table),
        RelationTarget::Unknown(_) => Vec::new(),
    }
}

fn source_columns(lookup: &Lookup<'_>, source: &str, table: &str) -> Vec<Column> {
    lookup
        .ctx
        .source_detail_map
        .get(source)
        .and_then(|s| s.tables.get(table))
        .map(|t| {
            t.columns
                .iter()
                .map(|c| Column {
                    name: c.name.clone(),
                    relation: format!("{source}.{table}"),
                    data_type: c.data_type.clone(),
                    description: c.description.clone(),
                    of: Some(ColumnOf::Source {
                        source: source.to_string(),
                        table: table.to_string(),
                    }),
                })
                .collect()
        })
        .unwrap_or_default()
}
