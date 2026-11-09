//! The pass driver signature. Stub: returns the plan untouched with an
//! empty trace, marked incomplete. Replaced by `feat/ir-passes`:
//! lower → canonicalise to fixpoint (stop on no change or a repeated
//! `plan_hash`) → MIR detect → LIR detect.

use cogitator_ir::hir::Hir;
use cogitator_ir::lir::Lir;
use cogitator_ir::mir::Mir;
use cogitator_ir::VerifyMode;

use super::budget::Budgets;
use super::trace::Trace;
use crate::cx::Cx;
use crate::findings::Finding;

#[derive(Debug)]
pub struct PassInput<'a> {
    pub hir: Option<&'a Hir>,
    pub mir: Mir,
    pub lir: Option<Lir>,
    pub verify: VerifyMode,
    pub budgets: Budgets,
}

#[derive(Debug)]
pub struct PassOutput {
    pub mir: Mir,
    pub lir: Option<Lir>,
    pub findings: Vec<Finding>,
    pub trace: Trace,
}

pub fn run_passes(input: PassInput<'_>, _cx: &Cx<'_>) -> PassOutput {
    PassOutput {
        mir: input.mir,
        lir: input.lir,
        findings: Vec::new(),
        trace: Trace {
            entries: Vec::new(),
            incomplete: true,
        },
    }
}
