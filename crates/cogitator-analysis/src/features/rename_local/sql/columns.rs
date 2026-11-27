//! Column aliases out of the bound HIR, for navigation only (renaming a
//! column is `rename_column`'s): every `… as col` of a select list, and
//! every name of a CTE's column list `with c (a, b) as (…)`, each with the
//! uses the binder resolved to it — a later clause reading the alias
//! (ORDER BY, GROUP BY, QUALIFY, a lateral alias), and a column of an
//! outer query that reads it through the CTE or the derived table
//! (`c.col`, or bare `col` bound to that FROM item).

use std::collections::HashMap;

use cogitator_ir::hir::{
    Body, ColRef, ColRefId, CteId, ExprId, ExprKind, FromId, FromKind, GroupBy, Hir, Ident,
    QueryId, RelTarget, Resolution, SelectItem,
};
use cogitator_ir::{Span, Symbol};

/// A select-list alias or a CTE column name, and where it is read.
#[derive(Debug, Clone)]
pub(crate) struct SqlColumn {
    pub definition: Span,
    pub uses: Vec<Span>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Key {
    /// Item `n` of the select list of a query's SELECT body.
    Item(QueryId, u32),
    /// Name `n` of a CTE's column list.
    CteCol(CteId, u32),
}

/// The SELECT a query's outputs are named by: its own, or the leftmost
/// of a set operation.
fn naming_select(hir: &Hir, mut q: QueryId) -> Option<QueryId> {
    for _ in 0..hir.queries.len() {
        match &hir.queries.get(q)?.body {
            Body::Select(_) => return Some(q),
            Body::SetOp { l, .. } => q = *l,
            _ => return None,
        }
    }
    None
}

/// The aliased item of `q`'s naming SELECT that is called `name`.
fn output_key(hir: &Hir, q: QueryId, name: Symbol) -> Option<Key> {
    let q = naming_select(hir, q)?;
    let Body::Select(select) = &hir.queries.get(q)?.body else {
        return None;
    };
    select
        .items
        .iter()
        .enumerate()
        .find_map(|(i, item)| match item {
            SelectItem::Expr { alias: Some(a), .. } if a.folded == name => {
                Some(Key::Item(q, i as u32))
            }
            _ => None,
        })
}

/// Where the column `name` read through FROM item `from` is named.
fn from_key(hir: &Hir, from: FromId, name: Symbol) -> Option<Key> {
    match &hir.from.get(from)?.kind {
        FromKind::Base {
            target: RelTarget::Cte(c),
            ..
        } => {
            let cte = hir.ctes.get(*c)?;
            if cte.columns.is_empty() {
                output_key(hir, cte.query, name)
            } else {
                let i = cte.columns.iter().position(|col| col.folded == name)?;
                Some(Key::CteCol(*c, i as u32))
            }
        }
        FromKind::Derived { q, .. } => output_key(hir, *q, name),
        _ => None,
    }
}

/// Where a reference stands: the query whose clauses hold it, and
/// whether in GROUP BY, HAVING or QUALIFY, which read select aliases.
#[derive(Debug, Clone, Copy)]
struct Owner {
    query: QueryId,
    late: bool,
}

/// Every column reference by the query that holds it (a subquery's are
/// its own).
fn owners(hir: &Hir) -> HashMap<ColRefId, Owner> {
    let mut out = HashMap::new();
    for (query, q) in hir.queries.iter() {
        let late: Vec<ExprId> = match &q.body {
            Body::Select(s) => {
                let mut late = match &s.group_by {
                    GroupBy::Exprs(e) | GroupBy::Rollup(e) | GroupBy::Cube(e) => e.clone(),
                    GroupBy::Sets(sets) => sets.iter().flatten().copied().collect(),
                    GroupBy::None | GroupBy::All => Vec::new(),
                };
                late.extend(s.having);
                late.extend(s.qualify);
                late
            }
            _ => Vec::new(),
        };
        let mut stack: Vec<(ExprId, bool)> = q
            .exprs()
            .into_iter()
            .map(|e| (e, late.contains(&e)))
            .collect();
        while let Some((id, late)) = stack.pop() {
            let Some(expr) = hir.exprs.get(id) else {
                continue;
            };
            if let ExprKind::Col(c) = expr.kind {
                out.insert(c, Owner { query, late });
            }
            stack.extend(expr.kind.children().into_iter().map(|e| (e, late)));
        }
    }
    out
}

/// Which column a reference reads, if it is one of ours.
fn key_of(hir: &Hir, col: &ColRef, owner: Option<Owner>) -> Option<Key> {
    let named = match col.res.as_ref()? {
        Resolution::LateralAlias(i) => {
            let q = owner?.query;
            return match &hir.queries.get(q)?.body {
                Body::Select(_) => Some(Key::Item(q, *i)),
                _ => {
                    let out = hir.queries.get(q)?.outputs.get(*i as usize)?;
                    output_key(hir, q, out.name?)
                }
            };
        }
        Resolution::Bound { from, col: index }
        | Resolution::Outer {
            from, col: index, ..
        } => {
            let name = hir.from.get(*from)?.output.get(*index as usize)?.name;
            return from_key(hir, *from, name);
        }
        Resolution::OpenSchema(from) => from_key(hir, *from, col.name.folded),
        // beside a relation of unknown columns: the one CTE or derived
        // table that names it, if only one does
        Resolution::Candidates(froms) => {
            let mut keys = froms
                .iter()
                .filter_map(|f| from_key(hir, *f, col.name.folded));
            keys.next().filter(|_| keys.next().is_none())
        }
        _ => None,
    };
    // `group by n` over a table of unknown columns: the binder cannot
    // tell `n` from an input column; a select alias of that name is
    // what such a clause almost always means
    named.or_else(|| {
        let owner = owner.filter(|o| o.late && col.qualifier.is_empty())?;
        output_key(hir, owner.query, col.name.folded)
    })
}

pub(super) fn collect(hir: &Hir, text: &str) -> Vec<SqlColumn> {
    let mut out = Vec::new();
    let mut index: HashMap<Key, usize> = HashMap::new();
    let mut add = |key: Key, ident: &Ident, out: &mut Vec<SqlColumn>| {
        index.insert(key, out.len());
        out.push(SqlColumn {
            definition: ident.span,
            uses: Vec::new(),
        });
    };
    for (qid, query) in hir.queries.iter() {
        if let Body::Select(select) = &query.body {
            for (i, item) in select.items.iter().enumerate() {
                if let SelectItem::Expr { alias: Some(a), .. } = item {
                    add(Key::Item(qid, i as u32), a, &mut out);
                }
            }
        }
    }
    for (cid, cte) in hir.ctes.iter() {
        for (i, col) in cte.columns.iter().enumerate() {
            add(Key::CteCol(cid, i as u32), col, &mut out);
        }
    }
    let owners = owners(hir);
    for (id, col) in hir.cols.iter() {
        let owner = owners.get(&id).copied();
        if let Some(&at) = key_of(hir, col, owner).and_then(|k| index.get(&k)) {
            out[at].uses.push(col.name.span);
        }
    }
    let valid = |s: Span| !s.is_empty() && s.is_valid_in(text);
    for column in &mut out {
        let definition = column.definition;
        column.uses.retain(|s| valid(*s) && *s != definition);
        column.uses.sort();
        column.uses.dedup();
    }
    out.retain(|c| valid(c.definition));
    out
}
