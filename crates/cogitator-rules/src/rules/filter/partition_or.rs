//! `filter.partition_or`: OR defeats partition pruning. (stub)

use cogitator_ir::lir::Lir;
use cogitator_ir::RuleId;

use crate::cx::Cx;
use crate::findings::Findings;
use crate::meta::{FixSafety, Need, Needs, RuleClass, RuleLevel, RuleMeta, Severity};
use crate::rule::{Detect, Rule};

pub static META: RuleMeta = RuleMeta {
    id: RuleId("filter.partition_or"),
    level: RuleLevel::Lir,
    class: RuleClass::Advice,
    needs: Needs::NONE.with(Need::Dialect).with(Need::Phys),
    severity: Severity::Warning,
    fix: FixSafety::None,
};

pub struct PartitionOr;

impl Rule for PartitionOr {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }
}

impl Detect<Lir> for PartitionOr {
    fn check(&self, _ir: &Lir, _cx: &Cx<'_>, _out: &mut Findings) {}
}
