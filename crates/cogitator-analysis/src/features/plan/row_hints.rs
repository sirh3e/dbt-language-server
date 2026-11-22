//! Row counts per CTE from catalog statistics (specs/ir-pipeline/06):
//! what the structure proves from the scanned tables' `row_count`, as an
//! exact count or an upper bound. Never a selectivity guess: a filter or
//! a group can only bound its input, a join of two tables proves
//! nothing, and a CTE whose rows reach no catalog statistic gets no hint.
//!
//! Pure over the MIR before the passes (they may inline CTEs away), the
//! physical facts and the HIR for the names' spans.

use std::collections::HashMap;

use cogitator_ir::common::{JoinKind, Lit, SetOpKind};
use cogitator_ir::hir::Hir;
use cogitator_ir::lir::PhysicalFacts;
use cogitator_ir::mir::{Mir, RelId, RelOp, ScalarId, ScalarKind};
use cogitator_ir::origin::SourceOrigin;
use cogitator_ir::{Span, Symbol};
use serde::Serialize;

/// What the statistics prove about a relation's row count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Rows {
    Exact(u64),
    AtMost(u64),
}

impl Rows {
    pub fn count(self) -> u64 {
        match self {
            Rows::Exact(n) | Rows::AtMost(n) => n,
        }
    }

    /// The same count, weakened to a bound.
    fn bound(self) -> Rows {
        Rows::AtMost(self.count())
    }

    /// The tighter of `self` and an upper bound `n`.
    fn capped(self, n: u64) -> Rows {
        match self {
            Rows::Exact(rows) if rows <= n => self,
            _ => Rows::AtMost(self.count().min(n)),
        }
    }
}

/// One CTE's proven row count, anchored at its name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RowHintView {
    pub cte: String,
    /// The CTE name's bytes in the model text.
    pub span: Span,
    pub rows: Rows,
}

/// Every CTE whose row count the statistics prove. Empty for a compiled
/// plan (its spans are the compiled file's) and without statistics.
pub(super) fn cte_rows(hir: &Hir, mir: &Mir, facts: &PhysicalFacts) -> Vec<RowHintView> {
    if !matches!(hir.origin, SourceOrigin::Template { .. }) {
        return Vec::new();
    }
    if facts.tables.values().all(|t| t.row_count.is_none()) {
        return Vec::new();
    }
    let spans = unique_cte_spans(hir);
    let mut walk = Walk {
        mir,
        facts,
        memo: HashMap::new(),
    };
    let mut out: Vec<RowHintView> = mir
        .ctes
        .iter()
        .filter_map(|(_, cte)| {
            let name = cte.name?;
            let span = *spans.get(&name)?;
            let rows = walk.rows(cte.body, 0)?;
            Some(RowHintView {
                cte: mir.syms.text(name).to_string(),
                span,
                rows,
            })
        })
        .collect();
    out.sort_by_key(|hint| hint.span);
    out
}

/// CTE names defined exactly once: a shadowed name cannot be placed.
fn unique_cte_spans(hir: &Hir) -> HashMap<Symbol, Span> {
    let mut seen: HashMap<Symbol, Option<Span>> = HashMap::new();
    for (_, cte) in hir.ctes.iter() {
        seen.entry(cte.name.folded)
            .and_modify(|slot| *slot = None)
            .or_insert(Some(cte.name.span));
    }
    seen.into_iter()
        .filter_map(|(name, span)| Some((name, span?)))
        .collect()
}

/// Deeper than any real model; stops a malformed plan from recursing on.
const MAX_DEPTH: usize = 256;

struct Walk<'a> {
    mir: &'a Mir,
    facts: &'a PhysicalFacts,
    /// `None` cached too: a CTE read twice is walked once.
    memo: HashMap<RelId, Option<Rows>>,
}

impl Walk<'_> {
    fn rows(&mut self, id: RelId, depth: usize) -> Option<Rows> {
        if depth > MAX_DEPTH {
            return None;
        }
        if let Some(known) = self.memo.get(&id) {
            return *known;
        }
        let rows = self.compute(id, depth + 1);
        self.memo.insert(id, rows);
        rows
    }

    fn compute(&mut self, id: RelId, depth: usize) -> Option<Rows> {
        let rel = self.mir.rels.get(id)?;
        match &rel.op {
            RelOp::Scan { table, .. } => self
                .facts
                .table(*table)
                .and_then(|t| t.row_count)
                .map(Rows::Exact),
            // row-preserving
            RelOp::Project { input, .. }
            | RelOp::Sort { input, .. }
            | RelOp::Window { input, .. } => self.rows(*input, depth),
            // row-reducing: only a bound survives
            RelOp::Filter { input, .. } | RelOp::Distinct(input) => {
                self.rows(*input, depth).map(Rows::bound)
            }
            RelOp::Aggregate {
                input, keys, sets, ..
            } => {
                let input = self.rows(*input, depth)?;
                // a global aggregate is one row, whatever it reads
                Some(if keys.is_empty() && sets.is_none() {
                    Rows::Exact(1)
                } else if sets.is_some() {
                    return None;
                } else {
                    input.bound()
                })
            }
            RelOp::Limit { input, limit, .. } => {
                let input = self.rows(*input, depth)?;
                Some(match limit.and_then(|l| self.literal(l)) {
                    Some(n) => input.bound().capped(n),
                    None => input.bound(),
                })
            }
            RelOp::Join { kind, l, .. } => match kind {
                JoinKind::Semi | JoinKind::Anti => self.rows(*l, depth).map(Rows::bound),
                // fan-out is unknown without keys; never guess
                _ => None,
            },
            RelOp::SetOp {
                op, all, inputs, ..
            } => self.set_op(*op, *all, inputs, depth),
            RelOp::CteRef { cte, .. } => {
                let body = self.mir.ctes.get(*cte)?.body;
                self.rows(body, depth)
            }
            RelOp::Unnest { .. } | RelOp::Opaque { .. } => None,
        }
    }

    fn set_op(&mut self, op: SetOpKind, all: bool, inputs: &[RelId], depth: usize) -> Option<Rows> {
        let rows: Vec<Rows> = inputs
            .iter()
            .map(|i| self.rows(*i, depth))
            .collect::<Option<_>>()?;
        let first = *rows.first()?;
        Some(match op {
            SetOpKind::Union => {
                let total = rows
                    .iter()
                    .fold(0u64, |acc, r| acc.saturating_add(r.count()));
                let exact = all && rows.iter().all(|r| matches!(r, Rows::Exact(_)));
                if exact {
                    Rows::Exact(total)
                } else {
                    Rows::AtMost(total)
                }
            }
            SetOpKind::Intersect => Rows::AtMost(rows.iter().map(|r| r.count()).min()?),
            SetOpKind::Except => first.bound(),
        })
    }

    /// A non-negative integer literal (`limit 10`).
    fn literal(&self, id: ScalarId) -> Option<u64> {
        match &self.mir.scalars.get(id)?.kind {
            ScalarKind::Lit(Lit::Number(n)) => self.mir.syms.text(*n).parse().ok(),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "row_hints_tests.rs"]
mod tests;
