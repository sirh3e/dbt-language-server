//! `incr.no_unique_key`: Merge strategy without a unique_key. (stub)

use cogitator_ir::hir::Hir;
use cogitator_ir::RuleId;

use crate::cx::Cx;
use crate::findings::Findings;
use crate::meta::{FixSafety, Need, Needs, RuleClass, RuleLevel, RuleMeta, Severity};
use crate::rule::{Detect, Rule};

pub static META: RuleMeta = RuleMeta {
    id: RuleId("incr.no_unique_key"),
    level: RuleLevel::Hir,
    class: RuleClass::Advice,
    needs: Needs::NONE.with(Need::Mat),
    severity: Severity::Warning,
    fix: FixSafety::None,
};

pub struct NoUniqueKey;

impl Rule for NoUniqueKey {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }
}

impl Detect<Hir> for NoUniqueKey {
    fn check(&self, _ir: &Hir, _cx: &Cx<'_>, _out: &mut Findings) {}
}
