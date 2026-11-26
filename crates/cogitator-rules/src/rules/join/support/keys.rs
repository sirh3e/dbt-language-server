//! Join keys and what is known of them: the equalities of an ON clause,
//! the scan behind a column, and whether a test or constraint proves a
//! column unique (`join.distinct_fanout`, `join.fanout`).

use std::collections::BTreeSet;

use cogitator_ir::common::BinaryOp;
use cogitator_ir::mir::{ColId, Mir, RelId, RelOp, ScalarKind, TableRef};

use crate::cx::Cx;
use crate::meta::Need;

/// `(left column, right column)` for each `a = b` tying the two sides.
pub(crate) fn equi_keys(
    mir: &Mir,
    on: &[cogitator_ir::mir::ScalarId],
    left: &BTreeSet<ColId>,
    right: &BTreeSet<ColId>,
) -> Vec<(ColId, ColId)> {
    let col = |id| match mir.scalars.get(id).map(|s| &s.kind) {
        Some(ScalarKind::Col(c)) => Some(*c),
        _ => None,
    };
    on.iter()
        .filter_map(|s| match mir.scalars.get(*s).map(|n| &n.kind) {
            Some(ScalarKind::Binary {
                op: BinaryOp::Eq,
                l,
                r,
            }) => {
                let (a, b) = (col(*l)?, col(*r)?);
                if left.contains(&a) && right.contains(&b) {
                    Some((a, b))
                } else if left.contains(&b) && right.contains(&a) {
                    Some((b, a))
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect()
}

/// True when `col` is a scan column with a `unique` test behind it. Only
/// a test counts as knowledge here, never the shape of the data.
pub(crate) fn column_is_unique(mir: &Mir, cx: &Cx<'_>, rel: RelId, col: ColId) -> bool {
    if !cx.has(Need::Tests) {
        return false;
    }
    let Some(schemas) = cx.schemas else {
        return false;
    };
    let Some((table, name)) = scan_of(mir, rel, col) else {
        return false;
    };
    let schema = match table {
        TableRef::Model { name, .. } => schemas.model(mir.syms.text(*name)),
        TableRef::Source { source, table } => {
            schemas.source(mir.syms.text(*source), mir.syms.text(*table))
        }
        _ => None,
    };
    schema.is_some_and(|s| {
        s.columns
            .iter()
            .any(|c| c.unique && c.name.eq_ignore_ascii_case(mir.syms.text(name)))
    })
}

/// The scan defining `col`, walking single-input operators and into the
/// join input that carries it. A renaming Project ends the walk: a
/// `unique` test on the source column says nothing about a new one.
pub(crate) fn scan_of(
    mir: &Mir,
    rel: RelId,
    col: ColId,
) -> Option<(&TableRef, cogitator_ir::Symbol)> {
    let mut at = rel;
    for _ in 0..=mir.rels.len() {
        match &mir.rels.get(at)?.op {
            RelOp::Scan { table, cols, .. } => {
                let name = mir.cols.get(col)?.name?;
                return cols
                    .contains(&col)
                    .then(|| Some((&mir.tables.get(*table)?.target, name)))?;
            }
            RelOp::Project { input, items } => {
                let (_, scalar) = items.iter().find(|(c, _)| *c == col)?;
                match mir.scalars.get(*scalar).map(|s| &s.kind) {
                    Some(ScalarKind::Col(c)) if *c == col => at = *input,
                    _ => return None,
                }
            }
            RelOp::Filter { input, .. }
            | RelOp::Sort { input, .. }
            | RelOp::Limit { input, .. }
            | RelOp::Distinct(input) => at = *input,
            RelOp::Join { l, r, .. } => {
                let in_left = mir.rels.get(*l)?.out.contains(&col);
                at = if in_left { *l } else { *r };
            }
            _ => return None,
        }
    }
    None
}
