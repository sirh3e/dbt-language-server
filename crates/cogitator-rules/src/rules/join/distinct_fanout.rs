//! `join.distinct_fanout`: DISTINCT may hide join fan-out (stub).

use cogitator_ir::mir::Mir;
use cogitator_ir::RuleId;

use crate::cx::Cx;
use crate::findings::Findings;
use crate::meta::{FixSafety, Need, Needs, RuleClass, RuleLevel, RuleMeta, Severity};
use crate::rule::{Detect, Rule};

pub static META: RuleMeta = RuleMeta {
    id: RuleId("join.distinct_fanout"),
    level: RuleLevel::Mir,
    class: RuleClass::Advice,
    needs: Needs::NONE.with(Need::Tests),
    severity: Severity::Hint,
    fix: FixSafety::None,
};

pub struct DistinctFanout;

impl Rule for DistinctFanout {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }
}

impl Detect<Mir> for DistinctFanout {
    fn check(&self, _ir: &Mir, _cx: &Cx<'_>, _out: &mut Findings) {}
}
