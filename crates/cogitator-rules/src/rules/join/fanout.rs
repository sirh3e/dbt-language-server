//! `join.fanout`: an equi-join whose key on the joined side is not
//! known to be unique, so each row of the other side may come out more
//! than once — and no GROUP BY (or DISTINCT) above restores the grain.
//!
//! Known-unique is a `unique` test or constraint (single or composite,
//! `SchemaCol::unique` / `ModelSchema::unique_sets`), or a shape that
//! gives one row per key: a GROUP BY on it, a DISTINCT, a
//! `row_number() … = 1` filter (`support::unique_on`). A one-to-many
//! join that is meant is still reported: the Hint is "not known", and a
//! test or the `cogitator: ignore` comment states the intent.

use std::collections::{BTreeSet, HashMap};

use cogitator_ir::common::JoinKind;
use cogitator_ir::mir::{ColId, Mir, RelId, RelOp, TableRef};
use cogitator_ir::{Level, RuleId};

use super::fanout_fix::dedup;
use super::support::{equi_keys, is_one_row, join_from, no_opaque, reads, scan_of, unique_on};
use crate::cx::Cx;
use crate::findings::{Evidence, Finding, Findings, RelationName, Subject};
use crate::meta::{FixSafety, Needs, RuleClass, RuleLevel, RuleMeta, Severity};
use crate::rule::{Detect, Rule};

pub static META: RuleMeta = RuleMeta {
    id: RuleId("join.fanout"),
    level: RuleLevel::Mir,
    class: RuleClass::Advice,
    needs: Needs::NONE,
    // a correctness hint on what is not known, never a claim
    severity: Severity::Hint,
    fix: FixSafety::ChangesIntent,
};

pub struct Fanout;

impl Rule for Fanout {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }
}

impl Detect<Mir> for Fanout {
    fn check(&self, ir: &Mir, cx: &Cx<'_>, out: &mut Findings) {
        let reachable = ir.reachable();
        let mut parents: HashMap<RelId, Vec<RelId>> = HashMap::new();
        for id in &reachable {
            if let Some(rel) = ir.rels.get(*id) {
                for input in rel.op.inputs() {
                    parents.entry(input).or_default().push(*id);
                }
            }
        }
        for id in reachable {
            if let Some(finding) = examine(ir, id, cx, &parents) {
                out.push(finding);
            }
        }
    }
}

fn examine(
    mir: &Mir,
    id: RelId,
    cx: &Cx<'_>,
    parents: &HashMap<RelId, Vec<RelId>>,
) -> Option<Finding> {
    let rel = mir.rels.get(id)?;
    let RelOp::Join { kind, l, r, on } = &rel.op else {
        return None;
    };
    if matches!(kind, JoinKind::Semi | JoinKind::Anti | JoinKind::Cross)
        || !no_opaque(mir, *l)
        || !no_opaque(mir, *r)
    {
        return None;
    }
    let left: BTreeSet<ColId> = mir.rels.get(*l)?.out.iter().copied().collect();
    let right: BTreeSet<ColId> = mir.rels.get(*r)?.out.iter().copied().collect();
    let keys = equi_keys(mir, on, &left, &right);
    if keys.is_empty() {
        return None;
    }
    // the joined side is the right one, but for a RIGHT JOIN
    let (driving, joined, mine, theirs): (RelId, RelId, Vec<ColId>, Vec<ColId>) = match kind {
        JoinKind::Right => (
            *r,
            *l,
            keys.iter().map(|k| k.1).collect(),
            keys.iter().map(|k| k.0).collect(),
        ),
        _ => (
            *l,
            *r,
            keys.iter().map(|k| k.0).collect(),
            keys.iter().map(|k| k.1).collect(),
        ),
    };
    let their_cols: BTreeSet<ColId> = theirs.iter().copied().collect();
    if is_one_row(mir, joined) || unique_on(mir, cx, joined, &their_cols) {
        return None;
    }
    let all_keys: BTreeSet<ColId> = mine.iter().chain(&theirs).copied().collect();
    if restored(mir, parents, id, &all_keys) {
        return None;
    }
    let key_names: Vec<String> = theirs
        .iter()
        .filter_map(|c| Some(mir.syms.text(mir.cols.get(*c)?.name?).to_string()))
        .collect();
    if key_names.len() != theirs.len() {
        return None;
    }
    let key = key_names.join(", ");
    let relation = label(mir, joined, theirs[0]);
    let side = label(mir, driving, mine[0]);
    let subject = match (
        traced_relation(mir, joined, theirs[0]),
        key_names.as_slice(),
    ) {
        (Some(relation), [column]) => Some(Subject::UniqueKey {
            relation,
            column: column.clone(),
        }),
        _ => None,
    };
    let fixes = match (cx.hir, cx.text, join_from(mir, rel.prov)) {
        (Some(hir), Some(text), Some(from)) => {
            dedup(hir, text, from, *kind, &key_names, cx.dialect)
                .into_iter()
                .collect()
        }
        _ => Vec::new(),
    };
    // "not known" is never a claim, whatever tests were read
    let evidence = Evidence::Heuristic;
    Some(Finding {
        id: META.id,
        span: mir.prov.span(rel.prov).unwrap_or_default(),
        level: Level::Mir,
        severity: META.severity,
        message: format!(
            "this join may duplicate rows of {side}: {key} is not known to be unique in {relation}"
        ),
        evidence,
        fixes,
        subject,
    })
}

/// A GROUP BY on a key column, or a DISTINCT, above the join (through
/// row-preserving operators and further joins) restores the grain.
fn restored(
    mir: &Mir,
    parents: &HashMap<RelId, Vec<RelId>>,
    join: RelId,
    keys: &BTreeSet<ColId>,
) -> bool {
    let mut at = join;
    for _ in 0..=mir.rels.len() {
        let Some([parent]) = parents.get(&at).map(Vec::as_slice) else {
            return false;
        };
        match mir.rels.get(*parent).map(|r| &r.op) {
            Some(RelOp::Aggregate { keys: group, .. }) => {
                return group
                    .iter()
                    .any(|(_, s)| reads(mir, *s).cols.iter().any(|c| keys.contains(c)));
            }
            Some(RelOp::Distinct(_)) => return true,
            Some(
                RelOp::Project { .. }
                | RelOp::Filter { .. }
                | RelOp::Sort { .. }
                | RelOp::Limit { .. }
                | RelOp::Window { .. }
                | RelOp::Join { .. },
            ) => at = *parent,
            _ => return false,
        }
    }
    false
}

/// The model or source `col` of `rel` is read from, through CTEs.
fn traced_relation(mir: &Mir, rel: RelId, col: ColId) -> Option<RelationName> {
    let (rel, col) = through_ctes(mir, rel, col)?;
    let (table, _) = scan_of(mir, rel, col)?;
    match table {
        TableRef::Model { name, .. } => Some(RelationName::Model(mir.syms.text(*name).to_string())),
        TableRef::Source { source, table } => Some(RelationName::Source {
            source: mir.syms.text(*source).to_string(),
            table: mir.syms.text(*table).to_string(),
        }),
        _ => None,
    }
}

/// Follows a column into the body of the CTE a pass-through chain
/// reads, as often as CTEs nest.
fn through_ctes(mir: &Mir, rel: RelId, col: ColId) -> Option<(RelId, ColId)> {
    let (mut rel, mut col) = (rel, col);
    for _ in 0..=mir.ctes.len() {
        match cte_ref(mir, rel, col) {
            Some((body, inner)) => (rel, col) = (body, inner),
            None => return Some((rel, col)),
        }
    }
    None
}

/// The CTE body and column a pass-through chain from `rel` reaches.
fn cte_ref(mir: &Mir, rel: RelId, col: ColId) -> Option<(RelId, ColId)> {
    let mut at = rel;
    for _ in 0..=mir.rels.len() {
        match &mir.rels.get(at)?.op {
            RelOp::CteRef { cte, out } => {
                let def = mir.ctes.get(*cte)?;
                let i = out.iter().position(|c| *c == col)?;
                return Some((def.body, *def.out.get(i)?));
            }
            RelOp::Filter { input, .. }
            | RelOp::Sort { input, .. }
            | RelOp::Limit { input, .. } => at = *input,
            _ => return None,
        }
    }
    None
}

/// How a message names the relation behind `col`: the model, the
/// source table, the CTE, or "a subquery".
fn label(mir: &Mir, rel: RelId, col: ColId) -> String {
    match traced_relation(mir, rel, col) {
        Some(RelationName::Model(name)) => return name,
        Some(RelationName::Source { source, table }) => return format!("{source}.{table}"),
        None => {}
    }
    let mut at = rel;
    for _ in 0..=mir.rels.len() {
        match mir.rels.get(at).map(|r| &r.op) {
            Some(RelOp::CteRef { cte, .. }) => {
                if let Some(name) = mir.ctes.get(*cte).and_then(|d| d.name) {
                    return mir.syms.text(name).to_string();
                }
                break;
            }
            Some(RelOp::Filter { input, .. } | RelOp::Sort { input, .. }) => at = *input,
            _ => break,
        }
    }
    "a subquery".to_string()
}
