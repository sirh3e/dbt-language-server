//! `cross.ephemeral_repeat`: Ephemeral model recomputed in each consumer. (stub)

use cogitator_ir::mir::Mir;
use cogitator_ir::RuleId;

use crate::cx::Cx;
use crate::findings::Findings;
use crate::meta::{FixSafety, Need, Needs, RuleClass, RuleLevel, RuleMeta, Severity};
use crate::rule::{Detect, Rule};

pub static META: RuleMeta = RuleMeta {
    id: RuleId("cross.ephemeral_repeat"),
    level: RuleLevel::Mir,
    class: RuleClass::Advice,
    needs: Needs::NONE.with(Need::Dag).with(Need::Mat),
    severity: Severity::Hint,
    fix: FixSafety::None,
};

pub struct EphemeralRepeat;

impl Rule for EphemeralRepeat {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }
}

impl Detect<Mir> for EphemeralRepeat {
    fn check(&self, _ir: &Mir, _cx: &Cx<'_>, _out: &mut Findings) {}
}
