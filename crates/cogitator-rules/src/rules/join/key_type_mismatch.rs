//! `join.key_type_mismatch`: Join keys have different types. (stub)

use cogitator_ir::mir::Mir;
use cogitator_ir::RuleId;

use crate::cx::Cx;
use crate::findings::Findings;
use crate::meta::{FixSafety, Need, Needs, RuleClass, RuleLevel, RuleMeta, Severity};
use crate::rule::{Detect, Rule};

pub static META: RuleMeta = RuleMeta {
    id: RuleId("join.key_type_mismatch"),
    level: RuleLevel::Mir,
    class: RuleClass::Advice,
    needs: Needs::NONE.with(Need::Types),
    severity: Severity::Info,
    fix: FixSafety::None,
};

pub struct KeyTypeMismatch;

impl Rule for KeyTypeMismatch {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }
}

impl Detect<Mir> for KeyTypeMismatch {
    fn check(&self, _ir: &Mir, _cx: &Cx<'_>, _out: &mut Findings) {}
}
