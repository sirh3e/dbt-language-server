//! `cross.duplicate_logic`: Same logic as another model. (stub)

use cogitator_ir::mir::Mir;
use cogitator_ir::RuleId;

use crate::cx::Cx;
use crate::findings::Findings;
use crate::meta::{FixSafety, Need, Needs, RuleClass, RuleLevel, RuleMeta, Severity};
use crate::rule::{Detect, Rule};

pub static META: RuleMeta = RuleMeta {
    id: RuleId("cross.duplicate_logic"),
    level: RuleLevel::Mir,
    class: RuleClass::Advice,
    needs: Needs::NONE.with(Need::Dag),
    severity: Severity::Hint,
    fix: FixSafety::None,
};

pub struct DuplicateLogic;

impl Rule for DuplicateLogic {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }
}

impl Detect<Mir> for DuplicateLogic {
    fn check(&self, _ir: &Mir, _cx: &Cx<'_>, _out: &mut Findings) {}
}
