//! `incr.subquery_watermark`: a subquery filter that does not prune (stub).

use cogitator_ir::hir::Hir;
use cogitator_ir::lir::Lir;
use cogitator_ir::RuleId;

use crate::cx::Cx;
use crate::findings::Findings;
use crate::meta::{FixSafety, Need, Needs, RuleClass, RuleLevel, RuleMeta, Severity};
use crate::rule::{Detect, Rule};

pub static META: RuleMeta = RuleMeta {
    id: RuleId("incr.subquery_watermark"),
    level: RuleLevel::HirAndLir,
    class: RuleClass::Advice,
    needs: Needs::NONE.with(Need::Dialect).with(Need::Phys),
    severity: Severity::Warning,
    fix: FixSafety::None,
};

pub struct SubqueryWatermark;

impl Rule for SubqueryWatermark {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }
}

impl Detect<Hir> for SubqueryWatermark {
    fn check(&self, _ir: &Hir, _cx: &Cx<'_>, _out: &mut Findings) {}
}

impl Detect<Lir> for SubqueryWatermark {
    fn check(&self, _ir: &Lir, _cx: &Cx<'_>, _out: &mut Findings) {}
}
