//! A query's clauses over one model's HIR: each clause's sites (byte
//! spans into the template), or none. Base relations are read as the
//! front end bound them (`RelTarget::Model`/`Source`); a CTE is no
//! relation of the project, so `reads` and `joins` see the refs inside
//! CTEs where they are written.

use cogitator_ir::hir::{
    ExprKind, From, FromId, FromKind, Hir, JoinCond, RelTarget, Resolution, SelectItem,
};
use cogitator_ir::{Span, Symbol};

use super::query::{Clause, Rel};

fn name(hir: &Hir, sym: Symbol) -> &str {
    hir.syms.resolve(sym).unwrap_or_default()
}

/// The Base relation `from` names, matched against `rel`.
fn base_matches(hir: &Hir, from: &From, rel: &Rel) -> bool {
    let FromKind::Base { target, .. } = &from.kind else {
        return false;
    };
    match target {
        RelTarget::Model { name: n, .. } => rel.accepts_model(name(hir, *n)),
        RelTarget::Source { source, table } => {
            rel.accepts_source(name(hir, *source), name(hir, *table))
        }
        _ => false,
    }
}

fn from_matches(hir: &Hir, id: FromId, rel: &Rel) -> bool {
    hir.from.get(id).is_some_and(|f| base_matches(hir, f, rel))
}

/// Every site of `clause` in `hir`.
pub(super) fn sites(hir: &Hir, clause: &Clause) -> Vec<Span> {
    match clause {
        Clause::Reads(rel) => hir
            .from
            .iter()
            .filter(|(_, f)| base_matches(hir, f, rel))
            .map(|(_, f)| f.span)
            .collect(),
        Clause::Joins { rel, on } => hir
            .from
            .iter()
            .filter_map(|(_, f)| match &f.kind {
                FromKind::Join { l, r, on: cond, .. } => {
                    let joined = from_matches(hir, *l, rel) || from_matches(hir, *r, rel);
                    let keyed = on.as_deref().is_none_or(|col| join_names(hir, cond, col));
                    (joined && keyed).then(|| hir.from.get(*r).map_or(f.span, |r| r.span))
                }
                _ => None,
            })
            .collect(),
        Clause::SelectStar { from } => stars(hir, from.as_ref()),
        Clause::UsesColumn { rel, column } => hir
            .cols
            .iter()
            .filter(|(_, c)| name(hir, c.name.folded).eq_ignore_ascii_case(column))
            .filter(|(_, c)| {
                let from = match &c.res {
                    Some(Resolution::Bound { from, .. })
                    | Some(Resolution::Outer { from, .. })
                    | Some(Resolution::OpenSchema(from)) => *from,
                    _ => return false,
                };
                from_matches(hir, from, rel)
            })
            .map(|(_, c)| c.span)
            .collect(),
    }
}

/// The join condition names `column`: an `ON` reading it, or `USING`.
fn join_names(hir: &Hir, cond: &JoinCond, column: &str) -> bool {
    match cond {
        JoinCond::Using(idents) => idents
            .iter()
            .any(|i| name(hir, i.folded).eq_ignore_ascii_case(column)),
        JoinCond::On(expr) => {
            let mut stack = vec![*expr];
            while let Some(id) = stack.pop() {
                let Some(e) = hir.exprs.get(id) else { continue };
                if let ExprKind::Col(c) = &e.kind {
                    let hit = hir
                        .cols
                        .get(*c)
                        .is_some_and(|c| name(hir, c.name.folded).eq_ignore_ascii_case(column));
                    if hit {
                        return true;
                    }
                }
                stack.extend(e.kind.children());
            }
            false
        }
        JoinCond::None | JoinCond::Natural => false,
    }
}

/// `*` items; with `from`, only those over a matching relation: `t.*`
/// by its FROM item, a bare `*` when its SELECT reads exactly one FROM
/// item and that one matches.
fn stars(hir: &Hir, rel: Option<&Rel>) -> Vec<Span> {
    let mut out = Vec::new();
    for (_, query) in hir.queries.iter() {
        let cogitator_ir::hir::Body::Select(select) = &query.body else {
            continue;
        };
        for item in &select.items {
            let SelectItem::Star { span, from, .. } = item else {
                continue;
            };
            let over = from.or_else(|| match select.from.as_slice() {
                [only] => Some(*only),
                _ => None,
            });
            let ok = match rel {
                None => true,
                Some(rel) => over.is_some_and(|f| from_matches(hir, f, rel)),
            };
            if ok {
                out.push(*span);
            }
        }
    }
    out
}
