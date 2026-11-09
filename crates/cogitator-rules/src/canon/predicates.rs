//! `canon.predicates`: predicate normalisation and pushdown (stub).

use cogitator_ir::mir::Mir;
use cogitator_ir::RuleId;

use crate::cx::Cx;
use crate::meta::{FixSafety, Needs, RuleClass, RuleLevel, RuleMeta, Severity};
use crate::rule::{Canonicalize, Rewrite, Rule};

pub static META: RuleMeta = RuleMeta {
    id: RuleId("canon.predicates"),
    level: RuleLevel::Mir,
    class: RuleClass::Canon,
    needs: Needs::NONE,
    severity: Severity::Off,
    fix: FixSafety::None,
};

pub struct Predicates;

impl Rule for Predicates {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }
}

impl Canonicalize for Predicates {
    fn apply(&self, ir: Mir, _cx: &Cx<'_>) -> Rewrite<Mir> {
        Rewrite::Unchanged(ir)
    }
}
