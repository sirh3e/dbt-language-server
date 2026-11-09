//! `subq.correlated`: Correlated subquery. (stub)

use cogitator_ir::mir::Mir;
use cogitator_ir::RuleId;

use crate::cx::Cx;
use crate::findings::Findings;
use crate::meta::{FixSafety, Needs, RuleClass, RuleLevel, RuleMeta, Severity};
use crate::rule::{Detect, Rule};

pub static META: RuleMeta = RuleMeta {
    id: RuleId("subq.correlated"),
    level: RuleLevel::Mir,
    class: RuleClass::Advice,
    needs: Needs::NONE,
    severity: Severity::Hint,
    fix: FixSafety::None,
};

pub struct Correlated;

impl Rule for Correlated {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }
}

impl Detect<Mir> for Correlated {
    fn check(&self, _ir: &Mir, _cx: &Cx<'_>, _out: &mut Findings) {}
}
