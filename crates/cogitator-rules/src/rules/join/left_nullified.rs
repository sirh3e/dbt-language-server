//! `join.left_nullified`: WHERE makes a LEFT JOIN inner (stub).

use cogitator_ir::mir::Mir;
use cogitator_ir::RuleId;

use crate::cx::Cx;
use crate::findings::Findings;
use crate::meta::{FixSafety, Needs, RuleClass, RuleLevel, RuleMeta, Severity};
use crate::rule::{Detect, Rule};

pub static META: RuleMeta = RuleMeta {
    id: RuleId("join.left_nullified"),
    level: RuleLevel::Mir,
    class: RuleClass::Advice,
    needs: Needs::NONE,
    severity: Severity::Warning,
    fix: FixSafety::Mixed,
};

pub struct LeftNullified;

impl Rule for LeftNullified {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }
}

impl Detect<Mir> for LeftNullified {
    fn check(&self, _ir: &Mir, _cx: &Cx<'_>, _out: &mut Findings) {}
}
