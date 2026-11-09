//! Pruning-key usage: classifies each predicate over a key and asks the
//! backend. Only filters directly above the scan decide `Used`/`Blocked`;
//! a key read by a filter higher up is `Unknown`, not `NotFiltered`.

use std::collections::{BTreeMap, BTreeSet};

use cogitator_ir::common::BinaryOp;
use cogitator_ir::lir::{KeyKind, PhysColumn, Pred, PredShape, PruneKey, PruneUse, Pruning};
use cogitator_ir::mir::{ColId, Mir, RelId, RelOp, ScalarId, ScalarKind, TableId};
use cogitator_ir::Symbol;

use super::Lowering;
use crate::DialectBackend;

/// What the plan adds to a `Pred` that its shape cannot say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PredDetail {
    None,
    /// `col + 1 > x`: the column is not isolated.
    ArithmeticOnColumn,
    /// `col = other_col`.
    ComparedToColumn,
}

pub struct PredContext<'a> {
    pub pred: Pred,
    pub col: &'a PhysColumn,
    /// The function name, when the shape is `Function`.
    pub function: Option<&'a str>,
    pub detail: PredDetail,
}

struct Classified {
    shape: PredShape,
    detail: PredDetail,
}

fn shape(shape: PredShape) -> Option<Classified> {
    Some(Classified {
        shape,
        detail: PredDetail::None,
    })
}

/// `pruning` plus `refine`: the full verdict for a predicate on `col`.
/// `None` when the predicate does not read `col`.
pub fn decide_pruning(
    backend: &dyn DialectBackend,
    mir: &Mir,
    scalar: ScalarId,
    col_id: ColId,
    col: &PhysColumn,
) -> Option<(Pred, Pruning)> {
    let classified = classify(mir, scalar, col_id)?;
    let pred = Pred {
        scalar,
        shape: classified.shape,
    };
    let function = match classified.shape {
        PredShape::Function { name } => mir.syms.resolve(name),
        _ => None,
    };
    let ctx = PredContext {
        pred,
        col,
        function,
        detail: classified.detail,
    };
    Some((pred, backend.refine(backend.pruning(&pred, col), &ctx)))
}

/// `Filter` conjuncts directly above each `Scan`, through stacked Filters.
pub(super) fn adjacent_filters(mir: &Mir, reachable: &[RelId]) -> BTreeMap<RelId, Vec<ScalarId>> {
    let mut map: BTreeMap<RelId, Vec<ScalarId>> = BTreeMap::new();
    for id in reachable {
        let Some(RelOp::Filter { input, conjuncts }) = mir.rels.get(*id).map(|r| &r.op) else {
            continue;
        };
        let mut below = *input;
        while let Some(RelOp::Filter { input, .. }) = mir.rels.get(below).map(|r| &r.op) {
            below = *input;
        }
        if matches!(mir.rels.get(below).map(|r| &r.op), Some(RelOp::Scan { .. })) {
            map.entry(below)
                .or_default()
                .extend(conjuncts.iter().copied());
        }
    }
    map
}

/// Every column any reachable `Filter` reads.
pub(super) fn columns_filtered(mir: &Mir, reachable: &[RelId]) -> BTreeSet<ColId> {
    let mut out = BTreeSet::new();
    for id in reachable {
        if let Some(RelOp::Filter { conjuncts, .. }) = mir.rels.get(*id).map(|r| &r.op) {
            for root in conjuncts {
                walk(mir, *root, &mut |kind| {
                    if let ScalarKind::Col(col) | ScalarKind::Outer(col) = kind {
                        out.insert(*col);
                    }
                });
            }
        }
    }
    out
}

fn walk(mir: &Mir, root: ScalarId, f: &mut impl FnMut(&ScalarKind)) {
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if let Some(scalar) = mir.scalars.get(id) {
            f(&scalar.kind);
            stack.extend(scalar.kind.children());
        }
    }
}

fn reads(mir: &Mir, root: ScalarId, col: ColId) -> bool {
    let mut hit = false;
    walk(mir, root, &mut |kind| match kind {
        ScalarKind::Col(c) | ScalarKind::Outer(c) => hit |= *c == col,
        ScalarKind::Subquery { correlated, .. } => hit |= correlated.contains(&col),
        _ => {}
    });
    hit
}

fn reads_other_column(mir: &Mir, root: ScalarId, col: ColId) -> bool {
    let mut hit = false;
    walk(mir, root, &mut |kind| {
        if let ScalarKind::Col(c) | ScalarKind::Outer(c) = kind {
            hit |= *c != col;
        }
    });
    hit
}

fn has_subquery(mir: &Mir, root: ScalarId) -> bool {
    let mut hit = false;
    walk(mir, root, &mut |kind| {
        hit |= matches!(kind, ScalarKind::Subquery { .. });
    });
    hit
}

fn classify(mir: &Mir, id: ScalarId, col: ColId) -> Option<Classified> {
    if !reads(mir, id, col) {
        return None;
    }
    match &mir.scalars.get(id)?.kind {
        ScalarKind::Binary {
            op: BinaryOp::Or,
            l,
            r,
        } => classify_or(mir, *l, *r, col),
        ScalarKind::Binary { op, l, r } if is_comparison(*op) => {
            classify_compare(mir, *op, *l, *r, col)
        }
        ScalarKind::InList { e, list, .. } => {
            let (mut set_subquery, mut set_col) = (false, false);
            for item in list {
                set_subquery |= has_subquery(mir, *item);
                set_col |= reads(mir, *item, col) || reads_other_column(mir, *item, col);
            }
            if set_subquery {
                shape(PredShape::Subquery)
            } else if set_col {
                Some(Classified {
                    shape: PredShape::Other,
                    detail: PredDetail::ComparedToColumn,
                })
            } else {
                classify_side(mir, *e, col, PredShape::BareCompare)
            }
        }
        ScalarKind::Subquery { .. } => shape(PredShape::Subquery),
        _ => shape(PredShape::Other),
    }
}

fn is_comparison(op: BinaryOp) -> bool {
    use BinaryOp as B;
    matches!(op, B::Eq | B::NotEq | B::Lt | B::LtEq | B::Gt | B::GtEq)
}

fn classify_or(mir: &Mir, l: ScalarId, r: ScalarId, col: ColId) -> Option<Classified> {
    if reads_other_column(mir, l, col) || reads_other_column(mir, r, col) {
        return shape(PredShape::OrAcrossColumns);
    }
    let prunable = |side: ScalarId| {
        classify(mir, side, col)
            .is_some_and(|c| matches!(c.shape, PredShape::BareCompare | PredShape::Range))
    };
    if prunable(l) && prunable(r) {
        shape(PredShape::Range)
    } else {
        shape(PredShape::Other)
    }
}

fn classify_compare(
    mir: &Mir,
    op: BinaryOp,
    l: ScalarId,
    r: ScalarId,
    col: ColId,
) -> Option<Classified> {
    let (side, other) = match (reads(mir, l, col), reads(mir, r, col)) {
        (true, false) => (l, r),
        (false, true) => (r, l),
        _ => return shape(PredShape::Other),
    };
    if has_subquery(mir, other) {
        return shape(PredShape::Subquery);
    }
    if reads_other_column(mir, other, col) {
        return Some(Classified {
            shape: PredShape::Other,
            detail: PredDetail::ComparedToColumn,
        });
    }
    let bare = match op {
        BinaryOp::Eq => PredShape::BareCompare,
        BinaryOp::NotEq => PredShape::Other,
        _ => PredShape::Range,
    };
    classify_side(mir, side, col, bare)
}

/// How the side holding `col` wraps it.
fn classify_side(mir: &Mir, side: ScalarId, col: ColId, bare: PredShape) -> Option<Classified> {
    match &mir.scalars.get(side)?.kind {
        ScalarKind::Col(c) if *c == col => shape(bare),
        ScalarKind::Cast { e, .. } if is_col(mir, *e, col) => shape(PredShape::Cast),
        ScalarKind::Call { name, .. } => shape(PredShape::Function { name: *name }),
        ScalarKind::Binary { op, .. } if !is_comparison(*op) => Some(Classified {
            shape: PredShape::Other,
            detail: PredDetail::ArithmeticOnColumn,
        }),
        _ => shape(PredShape::Other),
    }
}

fn is_col(mir: &Mir, id: ScalarId, col: ColId) -> bool {
    matches!(mir.scalars.get(id).map(|s| &s.kind), Some(ScalarKind::Col(c)) if *c == col)
}

impl Lowering<'_> {
    /// Keys the backend can use, in `TableFacts` order, restricted to
    /// columns this scan reads.
    pub(super) fn pruning_keys(
        &self,
        scan: RelId,
        table: TableId,
        cols: &[ColId],
    ) -> Vec<PruneKey> {
        let Some(facts) = self.phys.table(table) else {
            return Vec::new();
        };
        let allowed = self.backend.capabilities().pruning_keys;
        let mut keys = Vec::new();
        for column in &facts.columns {
            let Some(kind) = column.key.filter(|k| allowed.contains(k)) else {
                continue;
            };
            let Some(col) = self.scan_col(cols, column.name) else {
                continue;
            };
            keys.push(self.key(scan, col, kind, column));
        }
        self.backend.finish_keys(&mut keys);
        keys
    }

    fn scan_col(&self, cols: &[ColId], name: Symbol) -> Option<ColId> {
        cols.iter()
            .copied()
            .find(|c| self.mir.cols.get(*c).and_then(|def| def.name) == Some(name))
    }

    fn key(&self, scan: RelId, col: ColId, kind: KeyKind, column: &PhysColumn) -> PruneKey {
        let mut blocked = None;
        let mut unknown = false;
        let conjuncts = self.scan_filters.get(&scan).map_or(&[][..], Vec::as_slice);
        for scalar in conjuncts {
            let Some((_, verdict)) = decide_pruning(self.backend, self.mir, *scalar, col, column)
            else {
                continue;
            };
            match verdict {
                Pruning::Prunes => {
                    return PruneKey {
                        col,
                        kind,
                        usage: PruneUse::Used(*scalar),
                    }
                }
                Pruning::Blocks { why, .. } => {
                    blocked = blocked.or(Some(PruneUse::Blocked {
                        by: *scalar,
                        reason: why,
                    }));
                }
                Pruning::Unknown => unknown = true,
            }
        }
        let usage = match blocked {
            Some(usage) => usage,
            None if unknown => PruneUse::Unknown,
            None if self.filtered_elsewhere.contains(&col) => PruneUse::Unknown,
            None => PruneUse::NotFiltered,
        };
        PruneKey { col, kind, usage }
    }
}
