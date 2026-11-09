//! `filter.non_sargable`: a wrapped column may block pruning (stub).

use cogitator_ir::lir::Lir;
use cogitator_ir::RuleId;

use crate::cx::Cx;
use crate::findings::Findings;
use crate::meta::{FixSafety, Need, Needs, RuleClass, RuleLevel, RuleMeta, Severity};
use crate::rule::{Detect, Rule};

pub static META: RuleMeta = RuleMeta {
    id: RuleId("filter.non_sargable"),
    level: RuleLevel::Lir,
    class: RuleClass::Advice,
    needs: Needs::NONE
        .with(Need::Dialect)
        .with(Need::Types)
        .with(Need::Phys),
    severity: Severity::Warning,
    fix: FixSafety::Preserving,
};

pub struct NonSargable;

impl Rule for NonSargable {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }
}

impl Detect<Lir> for NonSargable {
    fn check(&self, _ir: &Lir, _cx: &Cx<'_>, _out: &mut Findings) {}
}
