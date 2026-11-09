//! Whether a query's output column list is complete. Shared with the
//! project-schema job, which uses it to decide `Completeness`.

use cogitator_ir::hir::{Body, FromId, FromKind, Hir, JinjaRole, QueryId, SelectItem};

/// Recursion bound for nested set operations.
const MAX_DEPTH: usize = 256;

/// True when the query may produce columns the HIR cannot name: a star
/// over an open FROM item, Jinja expanding to unknown columns, a select
/// item or body that did not parse. Open inputs read by named columns do
/// not count; those columns are written out.
pub fn output_open(hir: &Hir, query: QueryId) -> bool {
    open_at(hir, query, 0)
}

fn open_at(hir: &Hir, query: QueryId, depth: usize) -> bool {
    if depth > MAX_DEPTH {
        return true;
    }
    let Some(q) = hir.queries.get(query) else {
        return true;
    };
    match &q.body {
        Body::Select(select) => select.items.iter().any(|item| match item {
            SelectItem::Star { from, .. } => {
                let targets = from.map_or_else(|| select.from.clone(), |f| vec![f]);
                targets.is_empty() || targets.iter().any(|t| from_open(hir, *t))
            }
            SelectItem::Jinja(jinja) => !matches!(
                hir.jinja.get(*jinja).map(|n| n.role),
                Some(JinjaRole::Scalar | JinjaRole::Blank | JinjaRole::ClausePosition)
            ),
            SelectItem::Hole(_) => true,
            SelectItem::Expr { .. } => false,
        }),
        Body::SetOp { l, r, .. } => open_at(hir, *l, depth + 1) || open_at(hir, *r, depth + 1),
        Body::Values(_) => false,
        Body::Hole(_) => true,
    }
}

fn from_open(hir: &Hir, from: FromId) -> bool {
    let mut stack = vec![from];
    let mut budget = hir.from.len() + 1;
    while let Some(id) = stack.pop() {
        if budget == 0 {
            return true;
        }
        budget -= 1;
        match hir.from.get(id) {
            Some(item) => match &item.kind {
                FromKind::Join { l, r, .. } => {
                    stack.push(*l);
                    stack.push(*r);
                }
                _ if item.open => return true,
                _ => {}
            },
            None => return true,
        }
    }
    false
}
