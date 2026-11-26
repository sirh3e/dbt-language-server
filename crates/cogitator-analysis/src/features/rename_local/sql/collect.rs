//! CTEs and aliases out of the bound HIR: each with its definition span
//! and every use the binder resolved to it — a FROM/JOIN base naming the
//! CTE, and a qualified column `x.col` whose qualifier names the CTE or
//! the alias. A qualified column the binder left unresolved (a column the
//! schema does not list) is matched by its qualifier in the innermost
//! scope that has a FROM item of that name.

use cogitator_ir::hir::{ColRef, FromId, FromKind, Hir, Ident, RelTarget, Resolution};
use cogitator_ir::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SqlKind {
    Cte,
    Alias,
}

#[derive(Debug, Clone)]
pub(crate) struct SqlLocal {
    pub kind: SqlKind,
    /// The dialect-folded name.
    pub folded: String,
    pub definition: Span,
    pub uses: Vec<Span>,
    /// For an alias: the spans of the scopes its FROM item is visible in.
    pub scopes: Vec<Span>,
}

/// What a rename must not collide with, beside the locals themselves.
#[derive(Debug, Clone, Default)]
pub(crate) struct SqlLocals {
    pub locals: Vec<SqlLocal>,
    /// Folded single-part physical relations the file reads.
    pub physical: Vec<String>,
    /// Folded first qualifiers of every qualified column, with its span.
    pub qualifiers: Vec<(String, Span)>,
    /// Folded names every FROM item answers to, by the scope spans it is in.
    pub from_names: Vec<(String, Vec<Span>)>,
    /// Select-list aliases and CTE column names, for navigation only.
    pub columns: Vec<super::columns::SqlColumn>,
}

fn name_of(hir: &Hir, ident: &Ident) -> String {
    hir.syms.text(ident.folded).to_string() // boundary: owned result
}

/// The name a FROM item answers to, if any.
fn answers_to(hir: &Hir, id: FromId) -> Option<String> {
    match &hir.from.get(id)?.kind {
        FromKind::Base { alias: Some(a), .. }
        | FromKind::Derived { alias: Some(a), .. }
        | FromKind::Unnest { alias: Some(a), .. }
        | FromKind::TableFn { alias: Some(a), .. } => Some(name_of(hir, a)),
        FromKind::Base {
            target: RelTarget::Cte(c),
            ..
        } => hir.ctes.get(*c).map(|c| name_of(hir, &c.name)),
        FromKind::Base {
            target: RelTarget::Physical(path),
            ..
        } => path.last().map(|i| name_of(hir, i)),
        _ => None,
    }
}

fn scopes_of(hir: &Hir, id: FromId) -> Vec<Span> {
    hir.scopes
        .iter()
        .filter(|(_, s)| s.from.contains(&id))
        .map(|(_, s)| s.span)
        .collect()
}

/// The FROM item a single-qualifier column reads.
fn qualifier_from(hir: &Hir, col: &ColRef) -> Option<FromId> {
    let [q] = col.qualifier.as_slice() else {
        return None;
    };
    match &col.res {
        Some(Resolution::Bound { from, .. })
        | Some(Resolution::Outer { from, .. })
        | Some(Resolution::OpenSchema(from)) => return Some(*from),
        _ => {}
    }
    let wanted = name_of(hir, q);
    hir.scopes
        .iter()
        .filter(|(_, s)| s.span.contains(col.span))
        .min_by_key(|(_, s)| s.span.len())
        .and_then(|(_, s)| {
            s.from
                .iter()
                .copied()
                .find(|f| answers_to(hir, *f).as_deref() == Some(wanted.as_str()))
        })
}

pub(super) fn collect(hir: &Hir, text: &str) -> SqlLocals {
    let valid = |s: Span| !s.is_empty() && s.is_valid_in(text);
    let mut out = SqlLocals::default();
    let reads: Vec<(FromId, Span)> = hir
        .cols
        .iter()
        .filter_map(|(_, c)| Some((qualifier_from(hir, c)?, c.qualifier.first()?.span)))
        .collect();
    for (_, c) in hir.cols.iter() {
        if let Some(q) = c.qualifier.first() {
            out.qualifiers.push((name_of(hir, q), q.span));
        }
    }
    for (cte_id, cte) in hir.ctes.iter() {
        let mut uses = Vec::new();
        for (fid, from) in hir.from.iter() {
            if let FromKind::Base {
                target: RelTarget::Cte(c),
                alias,
            } = &from.kind
            {
                if *c != cte_id {
                    continue;
                }
                uses.push(from.span);
                if alias.is_none() {
                    uses.extend(reads.iter().filter(|(f, _)| *f == fid).map(|(_, s)| *s));
                }
            }
        }
        out.locals.push(SqlLocal {
            kind: SqlKind::Cte,
            folded: name_of(hir, &cte.name),
            definition: cte.name.span,
            uses,
            scopes: Vec::new(),
        });
    }
    for (fid, from) in hir.from.iter() {
        if let Some(name) = answers_to(hir, fid) {
            out.from_names.push((name, scopes_of(hir, fid)));
        }
        match &from.kind {
            FromKind::Base {
                target: RelTarget::Physical(path),
                ..
            } if path.len() == 1 => out.physical.push(name_of(hir, &path[0])),
            _ => {}
        }
        let alias = match &from.kind {
            FromKind::Base { alias: Some(a), .. }
            | FromKind::Derived { alias: Some(a), .. }
            | FromKind::Unnest { alias: Some(a), .. }
            | FromKind::TableFn { alias: Some(a), .. } => a,
            _ => continue,
        };
        out.locals.push(SqlLocal {
            kind: SqlKind::Alias,
            folded: name_of(hir, alias),
            definition: alias.span,
            uses: reads
                .iter()
                .filter(|(f, _)| *f == fid)
                .map(|(_, s)| *s)
                .collect(),
            scopes: scopes_of(hir, fid),
        });
    }
    for local in &mut out.locals {
        local.uses.retain(|s| valid(*s));
        local.uses.sort();
        local.uses.dedup();
    }
    out.locals.retain(|l| valid(l.definition));
    out
}
