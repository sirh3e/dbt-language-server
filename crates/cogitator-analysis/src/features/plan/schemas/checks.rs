//! The correctness checks that report under every perf profile
//! (`join.fanout` and the type checks, `diagnostics_config::SNAPSHOT_RULES`)
//! run here, in the schema job, on the MIR it builds anyway, so a pulled
//! diagnostic never plans a model for them (planning per pull doubled its
//! latency on the jaffle fixture). Their findings index the SAVED file's
//! text and are tagged with its hash: a document with unsaved edits shows
//! none until it is saved and the job runs again.

use std::hash::{DefaultHasher, Hash, Hasher};

use cogitator_dialects::Dialect;
use cogitator_ir::hir::Hir;
use cogitator_ir::mir::Mir;
use cogitator_rules::{Cx, Finding, Findings, Need, Needs, REGISTRY};
use cogitator_sql_front::SchemaEnv;

use super::super::diagnostics_config::SNAPSHOT_RULES;
use super::super::facts::backend_for;

/// One model's checks, over the text they were computed from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModelChecks {
    /// [`text_hash`] of the saved file the findings index.
    pub text_hash: u64,
    pub findings: Vec<Finding>,
}

/// The hash a document's text is compared with before its checks show.
pub fn text_hash(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// Runs the snapshot rules' MIR detectors over the canonical plan.
pub(super) fn run_checks(
    hir: &Hir,
    mir: &Mir,
    text: &str,
    dialect: &Dialect,
    env: &SchemaEnv,
) -> ModelChecks {
    let mut cx = Cx::new(Needs::NONE.with(Need::Tests).with(Need::Types));
    cx.hir = Some(hir);
    cx.text = Some(text);
    cx.dialect = Some(backend_for(dialect));
    cx.schemas = Some(env.schemas.as_ref());
    let mut out = Findings::new();
    for rule in REGISTRY
        .iter()
        .filter(|r| SNAPSHOT_RULES.contains(&r.meta.id.as_str()))
    {
        if let Some(detect) = rule.mir {
            detect.check(mir, &cx, &mut out);
        }
    }
    ModelChecks {
        text_hash: text_hash(text),
        findings: out.into_sorted(),
    }
}
