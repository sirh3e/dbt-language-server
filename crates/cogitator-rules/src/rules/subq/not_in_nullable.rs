//! `subq.not_in_nullable`: NOT IN over a nullable column can return nothing. (stub)

use cogitator_ir::mir::Mir;
use cogitator_ir::RuleId;

use crate::cx::Cx;
use crate::findings::Findings;
use crate::meta::{FixSafety, Needs, RuleClass, RuleLevel, RuleMeta, Severity};
use crate::rule::{Detect, Rule};

pub static META: RuleMeta = RuleMeta {
    id: RuleId("subq.not_in_nullable"),
    level: RuleLevel::Mir,
    class: RuleClass::Advice,
    needs: Needs::NONE,
    severity: Severity::Warning,
    fix: FixSafety::ChangesIntent,
};

pub struct NotInNullable;

impl Rule for NotInNullable {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }
}

impl Detect<Mir> for NotInNullable {
    fn check(&self, _ir: &Mir, _cx: &Cx<'_>, _out: &mut Findings) {}
}
