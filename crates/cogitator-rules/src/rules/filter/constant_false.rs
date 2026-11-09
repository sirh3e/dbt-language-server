//! `filter.constant_false`: Filter is always false. (stub)

use cogitator_ir::mir::Mir;
use cogitator_ir::RuleId;

use crate::cx::Cx;
use crate::findings::Findings;
use crate::meta::{FixSafety, Needs, RuleClass, RuleLevel, RuleMeta, Severity};
use crate::rule::{Detect, Rule};

pub static META: RuleMeta = RuleMeta {
    id: RuleId("filter.constant_false"),
    level: RuleLevel::Mir,
    class: RuleClass::Advice,
    needs: Needs::NONE,
    severity: Severity::Hint,
    fix: FixSafety::None,
};

pub struct ConstantFalse;

impl Rule for ConstantFalse {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }
}

impl Detect<Mir> for ConstantFalse {
    fn check(&self, _ir: &Mir, _cx: &Cx<'_>, _out: &mut Findings) {}
}
