//! `cross.select_star_chain`: only part of a star is used downstream (stub).

use cogitator_ir::mir::Mir;
use cogitator_ir::RuleId;

use crate::cx::Cx;
use crate::findings::Findings;
use crate::meta::{FixSafety, Need, Needs, RuleClass, RuleLevel, RuleMeta, Severity};
use crate::rule::{Detect, Rule};

pub static META: RuleMeta = RuleMeta {
    id: RuleId("cross.select_star_chain"),
    level: RuleLevel::Mir,
    class: RuleClass::Advice,
    needs: Needs::NONE
        .with(Need::Schema)
        .with(Need::Dag)
        .with(Need::Mat),
    severity: Severity::Info,
    fix: FixSafety::ChangesIntent,
};

pub struct SelectStarChain;

impl Rule for SelectStarChain {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }
}

impl Detect<Mir> for SelectStarChain {
    fn check(&self, _ir: &Mir, _cx: &Cx<'_>, _out: &mut Findings) {}
}
