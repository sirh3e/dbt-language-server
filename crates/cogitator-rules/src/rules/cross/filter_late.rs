//! `cross.filter_late`: Every consumer repeats the same filter. (stub)

use cogitator_ir::mir::Mir;
use cogitator_ir::RuleId;

use crate::cx::Cx;
use crate::findings::Findings;
use crate::meta::{FixSafety, Need, Needs, RuleClass, RuleLevel, RuleMeta, Severity};
use crate::rule::{Detect, Rule};

pub static META: RuleMeta = RuleMeta {
    id: RuleId("cross.filter_late"),
    level: RuleLevel::Mir,
    class: RuleClass::Advice,
    needs: Needs::NONE.with(Need::Dag).with(Need::Mat),
    severity: Severity::Info,
    fix: FixSafety::None,
};

pub struct FilterLate;

impl Rule for FilterLate {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }
}

impl Detect<Mir> for FilterLate {
    fn check(&self, _ir: &Mir, _cx: &Cx<'_>, _out: &mut Findings) {}
}
