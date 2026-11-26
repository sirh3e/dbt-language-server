//! `type.aggregate_text`: `sum`/`avg` (and the other numeric
//! aggregates) over a text column. Some warehouses refuse it, others
//! cast every value implicitly and fail on the first one that is not a
//! number — or silently skip it.

use cogitator_ir::mir::{Mir, RelOp};
use cogitator_ir::{Level, RuleId};

use super::{evidence, family, proven, severity, type_of, Family};
use crate::cx::Cx;
use crate::findings::{Finding, Findings};
use crate::meta::{FixSafety, Needs, RuleClass, RuleLevel, RuleMeta, Severity};
use crate::rule::{Detect, Rule};

pub static META: RuleMeta = RuleMeta {
    id: RuleId("type.aggregate_text"),
    level: RuleLevel::Mir,
    class: RuleClass::Advice,
    // unknown types stay silent; casts type an expression without a schema
    needs: Needs::NONE,
    severity: Severity::Warning,
    fix: FixSafety::None,
};

/// Aggregates whose argument must be a number.
const NUMERIC: &[&str] = &[
    "sum",
    "avg",
    "mean",
    "stddev",
    "stddev_pop",
    "stddev_samp",
    "variance",
    "var_pop",
    "var_samp",
    "median",
];

pub struct AggregateText;

impl Rule for AggregateText {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }
}

impl Detect<Mir> for AggregateText {
    fn check(&self, ir: &Mir, cx: &Cx<'_>, out: &mut Findings) {
        for id in ir.reachable() {
            let Some(RelOp::Aggregate { input, aggs, .. }) = ir.rels.get(id).map(|r| &r.op) else {
                continue;
            };
            for (_, call) in aggs {
                let name = ir.syms.text(call.func).to_ascii_lowercase();
                if !NUMERIC.contains(&name.as_str()) {
                    continue;
                }
                let Some(arg) = call.args.first() else {
                    continue;
                };
                if type_of(ir, *arg).and_then(family) != Some(Family::Text) {
                    continue;
                }
                let Some(scalar) = ir.scalars.get(*arg) else {
                    continue;
                };
                out.push(Finding {
                    id: META.id,
                    span: ir.prov.span(scalar.prov).unwrap_or_default(),
                    level: Level::Mir,
                    severity: severity(proven(ir, cx, *input, *arg)),
                    message: format!(
                        "{name}() over text: the warehouse must cast every value to a number \
                         and fails (or skips) on one that is not; cast the column or fix its type"
                    ),
                    evidence: evidence(cx),
                    fixes: Vec::new(),
                    subject: None,
                });
            }
        }
    }
}
