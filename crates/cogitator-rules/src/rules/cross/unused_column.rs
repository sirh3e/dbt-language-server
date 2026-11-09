//! `cross.unused_column`: No downstream use in this project. (stub)

use cogitator_ir::mir::Mir;
use cogitator_ir::RuleId;

use crate::cx::Cx;
use crate::findings::Findings;
use crate::meta::{FixSafety, Need, Needs, RuleClass, RuleLevel, RuleMeta, Severity};
use crate::rule::{Detect, Rule};

pub static META: RuleMeta = RuleMeta {
    id: RuleId("cross.unused_column"),
    level: RuleLevel::Mir,
    class: RuleClass::Advice,
    needs: Needs::NONE.with(Need::Dag).with(Need::Mat),
    severity: Severity::Hint,
    fix: FixSafety::None,
};

pub struct UnusedColumn;

impl Rule for UnusedColumn {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }
}

impl Detect<Mir> for UnusedColumn {
    fn check(&self, _ir: &Mir, _cx: &Cx<'_>, _out: &mut Findings) {}
}
