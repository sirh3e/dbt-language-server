//! The `incrementality` section of `dbt.queryPlan` (specs/ir-pipeline/10):
//! for a `table` model, whether it could be built incrementally — the
//! strategy and what drives it — or every reason it has to stay a table,
//! each at its place in the model, as a warehouse's refresh-mode reason.

use cogitator_ir::mir::Mir;
use cogitator_rules::canon::incrementality::annotate::analysis_sexp;
use cogitator_rules::canon::incrementality::frontier::{frontier, frontier_sexp};
use cogitator_rules::canon::incrementality::{analyse, IncrementalInputs, Strategy, Verdict};
use cogitator_rules::rules::incr::could_be_incremental::describe;
use serde::Serialize;

use cogitator_ir::Span;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncrementalityView {
    /// `append`, `byBucket`, `byKey`, `keep`, `static` or `unknown`.
    pub verdict: &'static str,
    pub strategy: Option<&'static str>,
    pub unique_key: Vec<String>,
    /// The inputs whose new rows drive each run.
    pub inputs: Vec<DeltaView>,
    /// Why it stays a table: empty unless `verdict` is `keep`.
    pub reasons: Vec<ReasonView>,
    /// The finding's text, when there is an increment to offer.
    pub summary: Option<String>,
    /// The analysis as an S-expression: the verdict and every plan node's
    /// DBSP class (specs/ir-pipeline/10 §6, format 1).
    pub sexp: String,
    /// For a model that stays a table: the parts that would not, and the
    /// ones refused (format 2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontier: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeltaView {
    pub relation: String,
    pub column: String,
    pub declared: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReasonView {
    pub reason: String,
    pub span: Option<Span>,
}

impl IncrementalityView {
    pub(super) fn of(mir: &Mir, inputs: Option<&IncrementalInputs>, model: &str) -> Self {
        let analysis = analyse(mir, inputs);
        let summary = describe(&analysis);
        let sexp = analysis_sexp(mir, mir.root, model, &analysis).pretty(100);
        let frontier = matches!(analysis.verdict, Verdict::Keep(_))
            .then(|| frontier_sexp(mir, model, &frontier(mir, inputs)).pretty(100));
        let delta = |i: &cogitator_rules::canon::incrementality::DeltaInput| DeltaView {
            relation: i.relation.clone(),
            column: i.column.clone(),
            declared: i.declared,
        };
        let empty = |verdict| IncrementalityView {
            verdict,
            strategy: None,
            unique_key: Vec::new(),
            inputs: Vec::new(),
            reasons: Vec::new(),
            summary: None,
            sexp: sexp.clone(),
            frontier: frontier.clone(),
        };
        match analysis.verdict {
            Verdict::Append { inputs, .. } => IncrementalityView {
                strategy: Some("append"),
                inputs: inputs.iter().map(delta).collect(),
                summary,
                ..empty("append")
            },
            Verdict::ByBucket { input, keys, .. } => IncrementalityView {
                strategy: Some(Strategy::DeleteInsert.dbt_name()),
                unique_key: keys,
                inputs: vec![delta(&input)],
                summary,
                ..empty("byBucket")
            },
            Verdict::ByKey {
                input,
                keys,
                strategy,
                ..
            } => IncrementalityView {
                strategy: Some(strategy.dbt_name()),
                unique_key: keys,
                inputs: vec![delta(&input)],
                summary,
                ..empty("byKey")
            },
            Verdict::Keep(blocked) => IncrementalityView {
                reasons: blocked
                    .iter()
                    .map(|b| ReasonView {
                        reason: b.blocker.describe(),
                        span: mir.prov.span(b.prov),
                    })
                    .collect(),
                ..empty("keep")
            },
            Verdict::Static => empty("static"),
            Verdict::Unknown => empty("unknown"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::Facts;

    fn facts() -> Facts {
        Facts::new("duckdb").yaml(
            "events",
            &[
                ("id", Some("bigint")),
                ("ts", Some("timestamp")),
                ("a", Some("bigint")),
            ],
        )
    }

    const TABLE: &str = "{{ config(materialized='table') }}\n";

    #[test]
    fn a_table_that_must_stay_one_says_why_and_where() {
        let text = format!("{TABLE}select id, ts from {{{{ ref('events') }}}} order by ts limit 5");
        let plan = facts().plan(&text);
        let view = plan.incrementality.expect("a table model has the section");
        assert_eq!(view.verdict, "keep");
        assert_eq!(view.reasons.len(), 1, "{view:?}");
        assert!(view.reasons[0].reason.contains("LIMIT"), "{view:?}");
        let span = view.reasons[0].span.expect("placed");
        // the LIMIT's provenance is its query, after the config() line
        assert!(
            span.start as usize >= TABLE.len() && span.end as usize <= text.len(),
            "{span:?}"
        );
        assert_eq!(view.summary, None);
    }

    #[test]
    fn an_appendable_table_names_its_input_and_strategy() {
        let text = format!("{TABLE}select id, ts from {{{{ ref('events') }}}} where a > 0");
        let view = facts().plan(&text).incrementality.expect("section");
        assert_eq!(view.verdict, "append");
        assert_eq!(view.strategy, Some("append"));
        assert_eq!(view.inputs.len(), 1);
        assert_eq!(
            (
                view.inputs[0].relation.as_str(),
                view.inputs[0].column.as_str()
            ),
            ("events", "ts")
        );
        assert!(view.summary.is_some_and(|s| s.contains("append")));
    }

    #[test]
    fn only_table_models_have_the_section() {
        let view = "{{ config(materialized='view') }}\nselect id, ts from {{ ref('events') }}";
        assert!(facts().plan(view).incrementality.is_none());
    }
}
