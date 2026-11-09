//! `filter.cluster_order`: Filter skips the leading cluster column. (stub)

use cogitator_ir::lir::Lir;
use cogitator_ir::RuleId;

use crate::cx::Cx;
use crate::findings::Findings;
use crate::meta::{FixSafety, Need, Needs, RuleClass, RuleLevel, RuleMeta, Severity};
use crate::rule::{Detect, Rule};

pub static META: RuleMeta = RuleMeta {
    id: RuleId("filter.cluster_order"),
    level: RuleLevel::Lir,
    class: RuleClass::Advice,
    needs: Needs::NONE.with(Need::Dialect).with(Need::Phys),
    severity: Severity::Info,
    fix: FixSafety::None,
};

pub struct ClusterOrder;

impl Rule for ClusterOrder {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }
}

impl Detect<Lir> for ClusterOrder {
    fn check(&self, _ir: &Lir, _cx: &Cx<'_>, _out: &mut Findings) {}
}
