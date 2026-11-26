//! `type.text_temporal_compare`: text compared with a date or timestamp
//! in a WHERE, HAVING, ON or CASE. The warehouse casts one side
//! implicitly — the comparison then depends on the text's format, and
//! a text side usually defeats partition pruning. A string *literal* is
//! fine (`order_date >= '2024-01-01'` is the idiom) and stays silent.

use cogitator_ir::common::BinaryOp;
use cogitator_ir::mir::{Mir, RelId, RelOp, ScalarId, ScalarKind};
use cogitator_ir::{Level, RuleId};

use super::{evidence, family, proven, severity, subtree, type_of, Family};
use crate::cx::Cx;
use crate::findings::{Finding, Findings};
use crate::meta::{FixSafety, Needs, RuleClass, RuleLevel, RuleMeta, Severity};
use crate::rule::{Detect, Rule};

pub static META: RuleMeta = RuleMeta {
    id: RuleId("type.text_temporal_compare"),
    level: RuleLevel::Mir,
    class: RuleClass::Advice,
    // unknown types stay silent; casts type an expression without a schema
    needs: Needs::NONE,
    severity: Severity::Warning,
    fix: FixSafety::None,
};

pub struct TextTemporalCompare;

impl Rule for TextTemporalCompare {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }
}

impl Detect<Mir> for TextTemporalCompare {
    fn check(&self, ir: &Mir, cx: &Cx<'_>, out: &mut Findings) {
        for id in ir.reachable() {
            let Some(rel) = ir.rels.get(id) else {
                continue;
            };
            // the relation the compared columns are read from
            let reads_from = match &rel.op {
                RelOp::Join { .. } => id,
                op => match op.inputs().as_slice() {
                    [input] => *input,
                    _ => continue,
                },
            };
            for root in rel.op.scalars() {
                for scalar in subtree(ir, root) {
                    compared_pairs(ir, scalar)
                        .into_iter()
                        .filter_map(|(a, b)| mixed(ir, cx, reads_from, scalar, a, b))
                        .for_each(|f| out.push(f));
                }
            }
        }
    }
}

/// The operand pairs a scalar compares: a comparison's two sides, or a
/// simple CASE's operand against each WHEN value.
fn compared_pairs(mir: &Mir, id: ScalarId) -> Vec<(ScalarId, ScalarId)> {
    match mir.scalars.get(id).map(|s| &s.kind) {
        Some(ScalarKind::Binary {
            op:
                BinaryOp::Eq
                | BinaryOp::NotEq
                | BinaryOp::Lt
                | BinaryOp::LtEq
                | BinaryOp::Gt
                | BinaryOp::GtEq,
            l,
            r,
        }) => {
            vec![(*l, *r)]
        }
        Some(ScalarKind::Case {
            operand: Some(operand),
            arms,
            ..
        }) => arms.iter().map(|(when, _)| (*operand, *when)).collect(),
        _ => Vec::new(),
    }
}

fn is_literal(mir: &Mir, id: ScalarId) -> bool {
    matches!(
        mir.scalars.get(id).map(|s| &s.kind),
        Some(ScalarKind::Lit(_))
    )
}

fn mixed(
    mir: &Mir,
    cx: &Cx<'_>,
    rel: RelId,
    at: ScalarId,
    a: ScalarId,
    b: ScalarId,
) -> Option<Finding> {
    if is_literal(mir, a) || is_literal(mir, b) {
        return None;
    }
    let (fa, fb) = (family(type_of(mir, a)?)?, family(type_of(mir, b)?)?);
    let (text, temporal) = match (fa, fb) {
        (Family::Text, Family::Temporal) => (a, b),
        (Family::Temporal, Family::Text) => (b, a),
        _ => return None,
    };
    let all_proven = proven(mir, cx, rel, text) && proven(mir, cx, rel, temporal);
    let scalar = mir.scalars.get(at)?;
    Some(Finding {
        id: META.id,
        span: mir.prov.span(scalar.prov).unwrap_or_default(),
        level: Level::Mir,
        severity: severity(all_proven),
        message: "This compares text with a date or time: the implicit cast depends on the \
                  text's format and can defeat pruning; cast the text side explicitly"
            .to_owned(),
        evidence: evidence(cx),
        fixes: Vec::new(),
        subject: None,
    })
}
