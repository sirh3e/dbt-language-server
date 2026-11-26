//! The grain of a relation: whether its rows are unique on a set of
//! columns, from tests and constraints on the scans below it, and from
//! the shapes that produce one row per key (a GROUP BY, a DISTINCT, a
//! `row_number() … = 1` filter). Conservative: what it cannot prove is
//! not unique (`join.fanout`).

use std::collections::BTreeSet;

use cogitator_ir::common::{BinaryOp, Lit};
use cogitator_ir::mir::{ColId, Mir, RelId, RelOp, ScalarId, ScalarKind, TableRef};

use super::reads;
use crate::cx::Cx;
use crate::meta::Need;

/// True when `rel` has at most one row per value of `cols`.
pub(crate) fn unique_on(mir: &Mir, cx: &Cx<'_>, rel: RelId, cols: &BTreeSet<ColId>) -> bool {
    let mut at = rel;
    let mut cols = cols.clone();
    for _ in 0..=mir.rels.len() {
        if cols.is_empty() {
            return false;
        }
        let Some(node) = mir.rels.get(at) else {
            return false;
        };
        match &node.op {
            RelOp::Scan { table, .. } => {
                let Some(def) = mir.tables.get(*table) else {
                    return false;
                };
                return scan_unique(mir, cx, &def.target, &cols);
            }
            RelOp::Project { input, items } => {
                // pass-through columns keep their ids; anything computed
                // is dropped, asking for uniqueness on fewer columns
                cols = cols
                    .iter()
                    .filter(|c| {
                        items.iter().any(|(out, s)| {
                            *out == **c
                                && matches!(
                                    mir.scalars.get(*s).map(|x| &x.kind),
                                    Some(ScalarKind::Col(src)) if src == *c
                                )
                        })
                    })
                    .copied()
                    .collect();
                at = *input;
            }
            RelOp::Filter { input, conjuncts } => {
                if first_per_partition(mir, *input, conjuncts, &cols) {
                    return true;
                }
                at = *input;
            }
            RelOp::Sort { input, .. }
            | RelOp::Limit { input, .. }
            | RelOp::Window { input, .. } => at = *input,
            RelOp::Distinct(input) => {
                if node.out.iter().all(|c| cols.contains(c)) {
                    return true;
                }
                at = *input;
            }
            RelOp::Aggregate { keys, sets, .. } => {
                return sets.is_none() && keys.iter().all(|(c, _)| cols.contains(c));
            }
            RelOp::CteRef { cte, out } => {
                let Some(def) = mir.ctes.get(*cte) else {
                    return false;
                };
                cols = out
                    .iter()
                    .zip(&def.out)
                    .filter(|(o, _)| cols.contains(o))
                    .map(|(_, inner)| *inner)
                    .collect();
                at = def.body;
            }
            _ => return false,
        }
    }
    false
}

/// A unique test or constraint on a column of `cols`, or a composite key
/// whose columns are all among them. Only tests and constraints count.
fn scan_unique(mir: &Mir, cx: &Cx<'_>, target: &TableRef, cols: &BTreeSet<ColId>) -> bool {
    if !cx.has(Need::Tests) {
        return false;
    }
    let Some(schemas) = cx.schemas else {
        return false;
    };
    let schema = match target {
        TableRef::Model { name, .. } => schemas.model(mir.syms.text(*name)),
        TableRef::Source { source, table } => {
            schemas.source(mir.syms.text(*source), mir.syms.text(*table))
        }
        _ => None,
    };
    let Some(schema) = schema else {
        return false;
    };
    let names: Vec<String> = cols
        .iter()
        .filter_map(|c| mir.cols.get(*c)?.name)
        .map(|n| mir.syms.text(n).to_ascii_lowercase())
        .collect();
    let single = schema
        .columns
        .iter()
        .any(|c| c.unique && names.iter().any(|n| c.name.eq_ignore_ascii_case(n)));
    single
        || schema
            .unique_sets
            .iter()
            .any(|set| set.iter().all(|k| names.iter().any(|n| n == k.as_ref())))
}

/// `qualify row_number() over (partition by p …) = 1` (or a filter on
/// its alias): one row per `p`, so unique on `cols` ⊇ `p`.
fn first_per_partition(
    mir: &Mir,
    input: RelId,
    conjuncts: &[ScalarId],
    cols: &BTreeSet<ColId>,
) -> bool {
    let firsts: Vec<ColId> = conjuncts
        .iter()
        .filter_map(|c| equals_one(mir, *c))
        .collect();
    if firsts.is_empty() {
        return false;
    }
    let mut at = input;
    for _ in 0..=mir.rels.len() {
        match mir.rels.get(at).map(|r| &r.op) {
            Some(RelOp::Window { funcs, .. }) => {
                return funcs.iter().any(|(out, call)| {
                    firsts.contains(out)
                        && mir.syms.text(call.func).eq_ignore_ascii_case("row_number")
                        && !call.partition_by.is_empty()
                        && call.partition_by.iter().all(|p| {
                            let read = reads(mir, *p);
                            !read.opaque && read.cols.iter().all(|c| cols.contains(c))
                        })
                });
            }
            Some(RelOp::Project { input, .. }) | Some(RelOp::Sort { input, .. }) => at = *input,
            _ => return false,
        }
    }
    false
}

/// The column of `col = 1` (either way round), or `col <= 1`.
fn equals_one(mir: &Mir, id: ScalarId) -> Option<ColId> {
    let ScalarKind::Binary { op, l, r } = &mir.scalars.get(id)?.kind else {
        return None;
    };
    let kind = |s: ScalarId| mir.scalars.get(s).map(|x| &x.kind);
    let one = |s: ScalarId| matches!(kind(s), Some(ScalarKind::Lit(Lit::Number(n))) if mir.syms.text(*n) == "1");
    let col = |s: ScalarId| match kind(s) {
        Some(ScalarKind::Col(c)) => Some(*c),
        _ => None,
    };
    match op {
        BinaryOp::Eq if one(*r) => col(*l),
        BinaryOp::Eq if one(*l) => col(*r),
        BinaryOp::LtEq if one(*r) => col(*l),
        _ => None,
    }
}
