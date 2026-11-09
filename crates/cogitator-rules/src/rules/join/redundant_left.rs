//! `join.redundant_left`: LEFT JOIN adds nothing. (stub)

use cogitator_ir::mir::Mir;
use cogitator_ir::RuleId;

use crate::cx::Cx;
use crate::findings::Findings;
use crate::meta::{FixSafety, Need, Needs, RuleClass, RuleLevel, RuleMeta, Severity};
use crate::rule::{Detect, Rule};

pub static META: RuleMeta = RuleMeta {
    id: RuleId("join.redundant_left"),
    level: RuleLevel::Mir,
    class: RuleClass::Advice,
    needs: Needs::NONE.with(Need::Tests),
    severity: Severity::Info,
    fix: FixSafety::None,
};

pub struct RedundantLeft;

impl Rule for RedundantLeft {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }
}

impl Detect<Mir> for RedundantLeft {
    fn check(&self, _ir: &Mir, _cx: &Cx<'_>, _out: &mut Findings) {}
}
