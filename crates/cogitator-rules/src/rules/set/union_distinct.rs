//! `set.union_distinct`: UNION dedupes; UNION ALL? (stub).

use cogitator_ir::mir::Mir;
use cogitator_ir::RuleId;

use crate::cx::Cx;
use crate::findings::Findings;
use crate::meta::{FixSafety, Needs, RuleClass, RuleLevel, RuleMeta, Severity};
use crate::rule::{Detect, Rule};

pub static META: RuleMeta = RuleMeta {
    id: RuleId("set.union_distinct"),
    level: RuleLevel::Mir,
    class: RuleClass::Advice,
    needs: Needs::NONE,
    severity: Severity::Info,
    fix: FixSafety::Mixed,
};

pub struct UnionDistinct;

impl Rule for UnionDistinct {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }
}

impl Detect<Mir> for UnionDistinct {
    fn check(&self, _ir: &Mir, _cx: &Cx<'_>, _out: &mut Findings) {}
}
