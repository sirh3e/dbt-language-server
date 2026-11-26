//! `join.distinct_fanout`: DISTINCT over a join on a key not known to be
//! unique (DuckDB `join_elimination.cpp`, DataFusion
//! `replace_distinct_aggregate.rs`).
//!
//! `unique` tests are optional evidence, so `Need::Tests` is not declared:
//! a project without tests still gets the Hint. Tests come from YAML through
//! `ProjectSchemas`; the finding names them with `Evidence::Tested`.

use std::collections::BTreeSet;

use cogitator_ir::common::JoinKind;
use cogitator_ir::mir::{ColId, Mir, RelId, RelOp};
use cogitator_ir::{Level, RuleId};

use super::support::{column_is_unique, equi_keys, is_closed, reads};
use crate::cx::Cx;
use crate::findings::{Evidence, Finding, Findings};
use crate::meta::{FixSafety, Needs, RuleClass, RuleLevel, RuleMeta, Severity};
use crate::rule::{Detect, Rule};

pub static META: RuleMeta = RuleMeta {
    id: RuleId("join.distinct_fanout"),
    level: RuleLevel::Mir,
    class: RuleClass::Advice,
    needs: Needs::NONE,
    // Info is the most this reports, and only with a `unique` test behind it.
    severity: Severity::Info,
    fix: FixSafety::None,
};

pub struct DistinctFanout;

impl Rule for DistinctFanout {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }
}

impl Detect<Mir> for DistinctFanout {
    fn check(&self, ir: &Mir, cx: &Cx<'_>, out: &mut Findings) {
        for id in ir.reachable() {
            if let Some(finding) = examine(ir, id, cx) {
                out.push(finding);
            }
        }
    }
}

fn examine(mir: &Mir, id: RelId, cx: &Cx<'_>) -> Option<Finding> {
    let rel = mir.rels.get(id)?;
    let RelOp::Distinct(input) = &rel.op else {
        return None;
    };
    let (join, kept) = join_below(mir, *input, &rel.out)?;
    let RelOp::Join { kind, l, r, on } = &mir.rels.get(join)?.op else {
        return None;
    };
    if matches!(kind, JoinKind::Semi | JoinKind::Anti | JoinKind::Cross)
        || !is_closed(mir, *l)
        || !is_closed(mir, *r)
    {
        return None;
    }
    let (left, right) = (columns(mir, *l)?, columns(mir, *r)?);
    // A DISTINCT over columns of both sides keeps distinct pairs on purpose.
    if !(kept.is_subset(&left) || kept.is_subset(&right)) || kept.is_empty() {
        return None;
    }
    let keys = equi_keys(mir, on, &left, &right);
    if keys.is_empty() {
        return None;
    }
    let (severity, evidence, message) = verdict(mir, cx, *l, *r, &keys)?;
    Some(Finding {
        id: META.id,
        span: mir.prov.span(rel.prov).unwrap_or_default(),
        level: Level::Mir,
        severity,
        message,
        evidence,
        fixes: Vec::new(),
        subject: None,
    })
}

type Verdict = (Severity, Evidence, String);

fn verdict(mir: &Mir, cx: &Cx<'_>, l: RelId, r: RelId, keys: &[(ColId, ColId)]) -> Option<Verdict> {
    let unique = |rel: RelId, side: fn(&(ColId, ColId)) -> ColId| {
        keys.iter().any(|k| column_is_unique(mir, cx, rel, side(k)))
    };
    let left_unique = unique(l, |k| k.0);
    let right_unique = unique(r, |k| k.1);
    match (left_unique, right_unique) {
        // Both keys unique: the join cannot fan out at all.
        (true, true) => None,
        (false, false) => Some((
            Severity::Hint,
            Evidence::Heuristic,
            "DISTINCT over this join may hide duplicate rows from join fan-out".to_owned(),
        )),
        _ => {
            let schema_revision = cx.schemas.map_or(0, |s| s.revision);
            Some((
                Severity::Info,
                Evidence::Tested { schema_revision },
                "DISTINCT over this join hides duplicate rows: a `unique` test covers only one \
                 side of the key"
                    .to_owned(),
            ))
        }
    }
}

/// The join under a DISTINCT, through Project, Filter and Sort, with the
/// join-output columns the DISTINCT's own columns are made from.
fn join_below(mir: &Mir, input: RelId, out: &[ColId]) -> Option<(RelId, BTreeSet<ColId>)> {
    let mut kept: BTreeSet<ColId> = out.iter().copied().collect();
    let mut at = input;
    for _ in 0..=mir.rels.len() {
        match &mir.rels.get(at)?.op {
            RelOp::Project { input, items } => {
                let mut next = BTreeSet::new();
                for (col, scalar) in items {
                    if kept.contains(col) {
                        next.extend(reads(mir, *scalar).cols);
                    }
                }
                kept = next;
                at = *input;
            }
            RelOp::Filter { input, .. } | RelOp::Sort { input, .. } => at = *input,
            RelOp::Join { .. } => return Some((at, kept)),
            _ => return None,
        }
    }
    None
}

fn columns(mir: &Mir, rel: RelId) -> Option<BTreeSet<ColId>> {
    Some(mir.rels.get(rel)?.out.iter().copied().collect())
}
