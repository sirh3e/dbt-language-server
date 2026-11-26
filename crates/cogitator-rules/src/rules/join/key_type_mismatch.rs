//! `join.key_type_mismatch`: join keys in incompatible type families
//! (DataFusion `analyzer/`).
//!
//! Implicit coercion means the query usually runs, so this is worded as a
//! likely pruning or correctness risk. INT against DECIMAL is the same
//! family, and anything `Unknown` is silent. A Warning when both key
//! types are proven (see `rules::types::proven`), an Info otherwise; it
//! reports outside the perf profile with the `type.*` rules.

use cogitator_ir::common::BinaryOp;
use cogitator_ir::mir::{Mir, RelId, RelOp, ScalarId, ScalarKind};
use cogitator_ir::{Level, RuleId, SqlType};

use crate::cx::Cx;
use crate::findings::{Finding, Findings};
use crate::meta::{FixSafety, Need, Needs, RuleClass, RuleLevel, RuleMeta, Severity};
use crate::rule::{Detect, Rule};
use crate::rules::types::{evidence, family, proven, severity};

pub static META: RuleMeta = RuleMeta {
    id: RuleId("join.key_type_mismatch"),
    level: RuleLevel::Mir,
    class: RuleClass::Advice,
    needs: Needs::NONE.with(Need::Types),
    severity: Severity::Warning,
    fix: FixSafety::None,
};

pub struct KeyTypeMismatch;

impl Rule for KeyTypeMismatch {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }
}

impl Detect<Mir> for KeyTypeMismatch {
    fn check(&self, ir: &Mir, cx: &Cx<'_>, out: &mut Findings) {
        for id in ir.reachable() {
            examine(ir, id, cx, out);
        }
    }
}

fn examine(mir: &Mir, id: RelId, cx: &Cx<'_>, out: &mut Findings) {
    let Some(RelOp::Join { on, .. }) = mir.rels.get(id).map(|r| &r.op) else {
        return;
    };
    for conjunct in on {
        if let Some(finding) = check_key(mir, id, *conjunct, cx) {
            out.push(finding);
        }
    }
}

fn check_key(mir: &Mir, join: RelId, id: ScalarId, cx: &Cx<'_>) -> Option<Finding> {
    let scalar = mir.scalars.get(id)?;
    let ScalarKind::Binary {
        op: BinaryOp::Eq,
        l,
        r,
    } = &scalar.kind
    else {
        return None;
    };
    let (left, left_ty) = column_type(mir, *l)?;
    let (right, right_ty) = column_type(mir, *r)?;
    if left == right {
        return None;
    }
    let (a, b) = (family(left_ty)?, family(right_ty)?);
    if a == b {
        return None;
    }
    Some(Finding {
        id: META.id,
        span: mir.prov.span(scalar.prov).unwrap_or_default(),
        level: Level::Mir,
        severity: severity(proven(mir, cx, join, *l) && proven(mir, cx, join, *r)),
        message: format!(
            "These join keys compare {} with {}: the implicit cast can defeat pruning or \
             misjoin values, so cast one side explicitly if this is intended",
            a.label(),
            b.label()
        ),
        evidence: evidence(cx),
        fixes: Vec::new(),
        subject: None,
    })
}

/// A column reference and its declared base type.
fn column_type(mir: &Mir, id: ScalarId) -> Option<(cogitator_ir::mir::ColId, &SqlType)> {
    let scalar = mir.scalars.get(id)?;
    match scalar.kind {
        ScalarKind::Col(col) => Some((col, &mir.cols.get(col)?.ty.ty)),
        _ => None,
    }
}
