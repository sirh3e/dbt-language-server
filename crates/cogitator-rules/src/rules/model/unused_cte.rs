//! `model.unused_cte`: a CTE that is never used (stub).

use cogitator_ir::hir::Hir;
use cogitator_ir::RuleId;

use crate::cx::Cx;
use crate::findings::Findings;
use crate::meta::{FixSafety, Need, Needs, RuleClass, RuleLevel, RuleMeta, Severity};
use crate::rule::{Detect, Rule};

pub static META: RuleMeta = RuleMeta {
    id: RuleId("model.unused_cte"),
    level: RuleLevel::Hir,
    class: RuleClass::Advice,
    needs: Needs::NONE.with(Need::AllBranches),
    severity: Severity::Warning,
    fix: FixSafety::Preserving,
};

pub struct UnusedCte;

impl Rule for UnusedCte {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }
}

impl Detect<Hir> for UnusedCte {
    fn check(&self, _ir: &Hir, _cx: &Cx<'_>, _out: &mut Findings) {}
}
