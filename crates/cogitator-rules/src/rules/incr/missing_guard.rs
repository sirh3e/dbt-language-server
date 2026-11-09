//! `incr.missing_guard`: reprocesses all input each run (stub).

use cogitator_ir::hir::Hir;
use cogitator_ir::RuleId;

use crate::cx::Cx;
use crate::findings::Findings;
use crate::meta::{FixSafety, Need, Needs, RuleClass, RuleLevel, RuleMeta, Severity};
use crate::rule::{Detect, Rule};

pub static META: RuleMeta = RuleMeta {
    id: RuleId("incr.missing_guard"),
    level: RuleLevel::Hir,
    class: RuleClass::Advice,
    needs: Needs::NONE.with(Need::Mat),
    severity: Severity::Warning,
    fix: FixSafety::None,
};

pub struct MissingGuard;

impl Rule for MissingGuard {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }
}

impl Detect<Hir> for MissingGuard {
    fn check(&self, _ir: &Hir, _cx: &Cx<'_>, _out: &mut Findings) {}
}
