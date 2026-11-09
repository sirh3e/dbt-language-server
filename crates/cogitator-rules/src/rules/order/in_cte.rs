//! `order.in_cte`: ORDER BY has no effect here. (stub)

use cogitator_ir::mir::Mir;
use cogitator_ir::RuleId;

use crate::cx::Cx;
use crate::findings::Findings;
use crate::meta::{FixSafety, Needs, RuleClass, RuleLevel, RuleMeta, Severity};
use crate::rule::{Detect, Rule};

pub static META: RuleMeta = RuleMeta {
    id: RuleId("order.in_cte"),
    level: RuleLevel::Mir,
    class: RuleClass::Advice,
    needs: Needs::NONE,
    severity: Severity::Info,
    fix: FixSafety::Preserving,
};

pub struct InCte;

impl Rule for InCte {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }
}

impl Detect<Mir> for InCte {
    fn check(&self, _ir: &Mir, _cx: &Cx<'_>, _out: &mut Findings) {}
}
