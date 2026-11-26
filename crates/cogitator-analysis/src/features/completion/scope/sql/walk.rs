//! Where the placeholder sits in a parsed query: which clause of which
//! SELECT, and what that SELECT reads. Containment is asked of the AST's
//! own printing (`Display`), so every expression shape counts without a
//! walker per variant.

use std::fmt::Display;

use sqlparser::ast::{Query, Select, SelectItem, SetExpr, TableAlias, TableFactor, TableWithJoins};

use super::text::PH;

/// The clause the cursor is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Clause {
    /// A select list item (not its alias).
    Projection,
    Where,
    GroupBy,
    Having,
    Qualify,
    OrderBy,
    /// `JOIN … ON` / `USING`.
    JoinOn,
    /// The relation name after FROM / JOIN.
    Relation,
    /// Somewhere no column belongs: an alias being typed, a LIMIT, …
    Other,
}

impl Clause {
    /// Columns are what this clause is written of.
    pub(crate) fn takes_columns(self) -> bool {
        matches!(
            self,
            Clause::Projection
                | Clause::Where
                | Clause::GroupBy
                | Clause::Having
                | Clause::Qualify
                | Clause::OrderBy
                | Clause::JoinOn
        )
    }
}

pub(super) fn has(node: &impl Display) -> bool {
    node.to_string().contains(PH)
}

/// One FROM item of the cursor's SELECT, before its name is resolved.
#[derive(Debug, Clone)]
pub(super) enum Item<'q> {
    Table {
        name: &'q sqlparser::ast::ObjectName,
        alias: Option<&'q TableAlias>,
    },
    Derived {
        query: &'q Query,
        alias: &'q TableAlias,
    },
}

/// The cursor's clause and the SELECT it is in.
pub(super) struct Found<'q> {
    pub(super) clause: Clause,
    pub(super) select: Option<&'q Select>,
}

/// The FROM items of a SELECT, in order, nested joins flattened.
pub(super) fn items(select: &Select) -> Vec<Item<'_>> {
    let mut out = Vec::new();
    for twj in &select.from {
        twj_items(twj, &mut out);
    }
    out
}

fn twj_items<'q>(twj: &'q TableWithJoins, out: &mut Vec<Item<'q>>) {
    factor_items(&twj.relation, out);
    for join in &twj.joins {
        factor_items(&join.relation, out);
    }
}

fn factor_items<'q>(factor: &'q TableFactor, out: &mut Vec<Item<'q>>) {
    match factor {
        TableFactor::Table { name, alias, .. } => out.push(Item::Table {
            name,
            alias: alias.as_ref(),
        }),
        TableFactor::Derived {
            subquery,
            alias: Some(alias),
            ..
        } => out.push(Item::Derived {
            query: subquery,
            alias,
        }),
        TableFactor::NestedJoin {
            table_with_joins, ..
        } => twj_items(table_with_joins, out),
        _ => {}
    }
}

/// The placeholder's clause in `query`, `None` when it is not in it.
pub(super) fn find(query: &Query) -> Option<Found<'_>> {
    if let Some(with) = &query.with {
        if let Some(cte) = with.cte_tables.iter().find(|c| has(&c.query)) {
            return find(&cte.query);
        }
    }
    if let Some(found) = find_in_set(&query.body) {
        return Some(found);
    }
    let order_by = query.order_by.as_ref().is_some_and(has);
    let clause = if order_by {
        Clause::OrderBy
    } else if has(query) {
        Clause::Other
    } else {
        return None;
    };
    Some(Found {
        clause,
        select: first_select(&query.body),
    })
}

/// The SELECT whose FROM a query-level ORDER BY sees.
fn first_select(set: &SetExpr) -> Option<&Select> {
    match set {
        SetExpr::Select(select) => Some(select),
        SetExpr::Query(query) => first_select(&query.body),
        SetExpr::SetOperation { left, .. } => first_select(left),
        _ => None,
    }
}

fn find_in_set(set: &SetExpr) -> Option<Found<'_>> {
    match set {
        SetExpr::Select(select) if has(select) => Some(find_in_select(select)),
        SetExpr::Query(query) => find(query),
        SetExpr::SetOperation { left, right, .. } => {
            find_in_set(left).or_else(|| find_in_set(right))
        }
        _ => None,
    }
}

fn find_in_select(select: &Select) -> Found<'_> {
    let found = |clause| Found {
        clause,
        select: Some(select),
    };
    if let Some(item) = select.projection.iter().find(has) {
        return found(match item {
            SelectItem::ExprWithAlias { alias, .. } if has(alias) => Clause::Other,
            SelectItem::ExprWithAliases { aliases, .. } if aliases.iter().any(has) => Clause::Other,
            _ => Clause::Projection,
        });
    }
    for twj in select.from.iter().filter(has) {
        if let Some(nested) = in_factor(&twj.relation) {
            return nested.unwrap_or_else(|| found(factor_clause(&twj.relation)));
        }
        if let Some(join) = twj.joins.iter().find(has) {
            return match in_factor(&join.relation) {
                Some(nested) => nested.unwrap_or_else(|| found(factor_clause(&join.relation))),
                // the join holds it and its relation does not: ON / USING
                None => found(Clause::JoinOn),
            };
        }
    }
    let clause = if select.selection.as_ref().is_some_and(has) {
        Clause::Where
    } else if has(&select.group_by) {
        Clause::GroupBy
    } else if select.having.as_ref().is_some_and(has) {
        Clause::Having
    } else if select.qualify.as_ref().is_some_and(has) {
        Clause::Qualify
    } else {
        Clause::Other
    };
    found(clause)
}

/// `Some(Some(found))` when the placeholder is in a derived table's own
/// query, `Some(None)` when it is in this factor otherwise, `None` when
/// the factor does not hold it.
fn in_factor(factor: &TableFactor) -> Option<Option<Found<'_>>> {
    if !has(factor) {
        return None;
    }
    Some(match factor {
        TableFactor::Derived { subquery, .. } if has(subquery) => find(subquery),
        _ => None,
    })
}

/// A relation's name or an alias being typed.
fn factor_clause(factor: &TableFactor) -> Clause {
    match factor {
        TableFactor::Table { name, .. } if has(name) => Clause::Relation,
        _ => Clause::Other,
    }
}
