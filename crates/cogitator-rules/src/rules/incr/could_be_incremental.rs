//! `incr.could_be_incremental`: a `table` model, rebuilt in full on every
//! run, whose plan could be maintained incrementally
//! (specs/ir-pipeline/10, after DBSP — see `canon::incrementality`).
//!
//! It fires only on `materialized='table'`: a view is never built, and an
//! incremental model is incremental already. It says which dbt strategy
//! fits and what that relies on; where the input's growth was inferred
//! from a column name rather than declared to dbt, it is a hint and the
//! message says the assumption. A model that must stay a table gets no
//! finding — `dbt.queryPlan` shows why in its incrementality section.

use cogitator_ir::common::Materialization;
use cogitator_ir::mir::Mir;
use cogitator_ir::{Level, RuleId};

use super::missing_guard::anchor;
use crate::canon::incrementality::{analyse, Analysis, DeltaInput, Note, Strategy, Verdict};
use crate::cx::Cx;
use crate::findings::{Evidence, Finding, Findings};
use crate::meta::{FixSafety, Need, Needs, RuleClass, RuleLevel, RuleMeta, Severity};
use crate::rule::{Detect, Rule};

pub static META: RuleMeta = RuleMeta {
    id: RuleId("incr.could_be_incremental"),
    level: RuleLevel::Mir,
    class: RuleClass::Advice,
    needs: Needs::NONE.with(Need::Mat),
    severity: Severity::Info,
    fix: FixSafety::ChangesIntent,
};

pub struct CouldBeIncremental;

impl Rule for CouldBeIncremental {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }
}

impl Detect<Mir> for CouldBeIncremental {
    fn check(&self, ir: &Mir, cx: &Cx<'_>, out: &mut Findings) {
        if cx.mat != Some(Materialization::Table) {
            return;
        }
        let Some(hir) = cx.hir else {
            return;
        };
        let analysis = analyse(ir, cx.inputs);
        let Some(message) = describe(&analysis) else {
            return;
        };
        let declared = declared(&analysis);
        out.push(Finding {
            id: META.id,
            span: anchor(hir),
            level: Level::Mir,
            severity: if declared {
                Severity::Info
            } else {
                Severity::Hint
            },
            message,
            evidence: if declared {
                Evidence::Exact
            } else {
                Evidence::Heuristic
            },
            fixes: cx
                .text
                .and_then(|text| super::rewrite::rewrite(hir, text, &analysis.verdict))
                .into_iter()
                .collect(),
        });
    }
}

/// Every driving input's growth was declared to dbt.
fn declared(analysis: &Analysis) -> bool {
    match &analysis.verdict {
        Verdict::Append { inputs, .. } => inputs.iter().all(|i| i.declared),
        Verdict::ByBucket { input, .. } | Verdict::ByKey { input, .. } => input.declared,
        _ => false,
    }
}

/// The finding's text, or None where the verdict offers no increment.
pub fn describe(analysis: &Analysis) -> Option<String> {
    let mut text = match &analysis.verdict {
        Verdict::Append { inputs, out_column } => {
            let names: Vec<String> = inputs.iter().map(|i| format!("`{}`", i.relation)).collect();
            format!(
                "This table could be built incrementally with `incremental_strategy='append'`: \
                 it is linear in {} (filters, projections and joins to unchanging inputs), so \
                 each run needs only the rows added since the last, those with `{out_column}` \
                 after the newest `{out_column}` already in the model.",
                names.join(" and ")
            )
        }
        Verdict::ByBucket {
            bucket,
            keys,
            input,
        } => format!(
            "This table could be built incrementally with `incremental_strategy='delete+insert'` \
             and `unique_key={}`: it groups by `{bucket}`, a bucket of `{}.{}`, so new rows \
             change only the newest buckets, which each run recomputes and replaces.",
            key_list(keys),
            input.relation,
            input.column
        ),
        Verdict::ByKey {
            keys,
            strategy,
            high_water,
            input,
        } => {
            let mut text = format!(
                "This table could be built incrementally with `incremental_strategy='{}'` and \
                 `unique_key={}`: each run recomputes only the groups the new rows of `{}` \
                 touch, and replaces them.",
                strategy.dbt_name(),
                key_list(keys),
                input.relation
            );
            match (high_water, strategy) {
                (Some(column), _) => text.push_str(&format!(
                    " `{column}` holds each group's newest `{}`, which tells new rows apart.",
                    input.column
                )),
                (None, Strategy::DeleteInsert) => text.push_str(&format!(
                    " It needs a column with each group's newest `{0}` (`max({0})`) to tell new \
                     rows apart.",
                    input.column
                )),
                (None, Strategy::Merge) => {}
            }
            text
        }
        Verdict::Keep(_) | Verdict::Static | Verdict::Unknown => return None,
    };
    for input in inputs_of(&analysis.verdict) {
        if !input.declared {
            text.push_str(&format!(
                " Assumes `{0}` only gains rows, inferred from its column `{1}`; declaring \
                 `event_time: {1}` on it makes that explicit.",
                input.relation, input.column
            ));
        }
    }
    for note in &analysis.notes {
        text.push(' ');
        text.push_str(&note_text(note));
    }
    Some(text)
}

fn inputs_of(verdict: &Verdict) -> Vec<&DeltaInput> {
    match verdict {
        Verdict::Append { inputs, .. } => inputs.iter().collect(),
        Verdict::ByBucket { input, .. } | Verdict::ByKey { input, .. } => vec![input],
        _ => Vec::new(),
    }
}

fn key_list(keys: &[String]) -> String {
    match keys {
        [one] => format!("'{one}'"),
        many => format!(
            "[{}]",
            many.iter()
                .map(|k| format!("'{k}'"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn note_text(note: &Note) -> String {
    match note {
        Note::Bilinear { other } => format!(
            "Both join inputs grow: rows of `{other}` that arrive late match rows already \
             processed, so filter both inputs or allow a lookback."
        ),
        Note::AssumesStatic(name) => {
            format!("Assumes `{name}` does not change for rows already built.")
        }
        Note::Having => "HAVING can drop a group, so groups are replaced, not merged.".into(),
        Note::FloatSum => {
            "SUM/AVG over a float can differ by rounding from a full rebuild; a decimal does not."
                .into()
        }
        Note::MinMax => "MIN/MAX stay exact only while the input only gains rows.".into(),
        Note::Holistic(func) => format!(
            "`{func}` has no incremental form; it is exact here because touched groups are \
             recomputed whole."
        ),
    }
}
