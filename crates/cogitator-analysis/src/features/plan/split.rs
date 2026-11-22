//! Splitting a model at its incrementality frontier (specs/ir-pipeline/10
//! §6): a model that has to stay a table, with a CTE that would not —
//! reducing (aggregating, deduplicating, filtering) a growing input —
//! becomes two models. The CTE is extracted as `<model>__<cte>`, made
//! incremental by the same rewrite `incr.could_be_incremental` offers
//! (planned as a model of its own, so the fix is the one the oracle
//! holds to full rebuilds), and the model reads it instead.
//!
//! `Q = Q_keep ∘ Q_inc` becomes `Q_keep ∘ T`: `Q_inc` works on the new
//! rows only, `Q_keep` still rebuilds in full, but over `T`, which the
//! reduction makes smaller than what `Q_inc` reads. The plan comes as
//! S-expressions (formats 3 and 4), and as the files and edits that carry
//! it out. One cut per plan: the first CTE of the frontier.

use cogitator_ir::common::Materialization;
use cogitator_ir::sexp::{atom, list, Sexp};
use cogitator_lower::lower_to_mir;
use cogitator_lsp::TextEdit;
use cogitator_rules::canon::incrementality::frontier::{frontier, refusal_sexp, Refusal};
use cogitator_rules::canon::incrementality::{inputs_for, DeltaInput, Verdict};
use serde::Serialize;

use super::drive::{built_hir, merge_manifest_config};
use super::incremental::incremental_inputs;
use super::{query_plan, PlanInput};
use crate::features::code_actions::extract_cte::extraction;

const RULE: &str = "incr.could_be_incremental";

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitPlan {
    pub model: String,
    /// Format 3 (a split) or 4 (a refusal), pretty-printed.
    pub sexp: String,
    /// The new incremental model, when there is a split.
    pub part: Option<SplitPart>,
    /// The edits of the model itself: the CTE out, a `ref()` in.
    #[serde(skip)]
    pub edits: Vec<TextEdit>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitPart {
    pub name: String,
    pub cte: String,
    /// The new model's whole file.
    pub text: String,
}

/// The split of the model `input` plans, or None for a model that is no
/// table (nothing to split for) or that the front end cannot read.
pub fn split_plan(input: &PlanInput<'_>) -> Option<SplitPlan> {
    let model = input.model?;
    let mut hir = built_hir(input).hir.value?;
    merge_manifest_config(&mut hir, input);
    if hir.config.materialized != Some(Materialization::Table) {
        return None;
    }
    let mir = lower_to_mir(&hir).value?;
    let declared = incremental_inputs(input);
    let inputs = inputs_for(&hir, Some(&declared));
    let found = frontier(&mir, Some(&inputs));
    let refused = |refusals: &[Refusal]| SplitPlan {
        model: model.to_string(),
        sexp: list(
            "split",
            std::iter::once(atom(model)).chain(refusals.iter().map(refusal_sexp)),
        )
        .pretty(100),
        part: None,
        edits: Vec::new(),
    };
    let Some(cut) = found.cuts.first() else {
        return (!found.refusals.is_empty()).then(|| refused(&found.refusals));
    };
    let name = format!("{model}__{}", cut.cte);
    let mut docs = crate::Documents::default();
    docs.open("split:model".into(), input.text, input.dialect);
    let doc = docs.get("split:model")?;
    let Some(extracted) = extraction(doc, &cut.cte, &name) else {
        return Some(refused(&[Refusal::ReadsCte {
            cte: cut.cte.clone(),
        }]));
    };
    // the part as a table model, then the rule's own rewrite of it
    let table = format!(
        "{{{{ config(materialized='table') }}}}\n{}\n",
        extracted.body
    );
    let part_input = PlanInput {
        text: &table,
        model: Some(&name),
        ..*input
    };
    let fix = query_plan(&part_input)
        .findings
        .into_iter()
        .find(|f| f.id == RULE)
        .and_then(|f| f.fixes.into_iter().next());
    let Some(fix) = fix else {
        return Some(refused(&[Refusal::NotIncremental {
            cte: cut.cte.clone(),
        }]));
    };
    let mut text = table;
    for edit in fix.edits.iter().rev() {
        text.replace_range(
            edit.span.start as usize..edit.span.end as usize,
            &edit.new_text,
        );
    }
    Some(SplitPlan {
        model: model.to_string(),
        sexp: split_sexp(model, &name, &cut.cte, &cut.analysis.verdict, cut.reduces).pretty(100),
        part: Some(SplitPart {
            name,
            cte: cut.cte.clone(),
            text,
        }),
        edits: extracted.edits,
    })
}

/// Format 3: the incremental part, the model over it, and the cost.
fn split_sexp(model: &str, part: &str, cte: &str, verdict: &Verdict, reduces: &str) -> Sexp {
    let (strategy, keys, input, increment) = match verdict {
        Verdict::Append { inputs, out_column } => {
            let input = inputs.first();
            (
                "append",
                Vec::new(),
                input,
                input.map(|i| greater(i, out_column)),
            )
        }
        Verdict::ByBucket {
            input,
            bucket,
            keys,
        } => (
            "delete+insert",
            keys.clone(),
            Some(input),
            Some(list(
                "or",
                [
                    newest_is_null(bucket),
                    list(">=", [atom(&input.column), newest(bucket)]),
                ],
            )),
        ),
        Verdict::ByKey {
            input,
            keys,
            strategy,
            high_water,
        } => (
            strategy.dbt_name(),
            keys.clone(),
            Some(input),
            high_water.as_ref().and_then(|h| {
                let key = keys.first()?;
                Some(list(
                    "or",
                    [
                        newest_is_null(h),
                        list(
                            "in",
                            [
                                atom(key),
                                list(
                                    "select",
                                    [
                                        atom(key),
                                        list("scan", [atom(&input.relation)]),
                                        list(">", [atom(&input.column), newest(h)]),
                                    ],
                                ),
                            ],
                        ),
                    ],
                ))
            }),
        ),
        _ => ("none", Vec::new(), None, None),
    };
    let mut sub = vec![
        atom(part),
        list("materialized", [atom("incremental")]),
        list("strategy", [atom(strategy)]),
    ];
    if !keys.is_empty() {
        sub.push(list("unique-key", keys.iter().map(atom)));
    }
    sub.push(list("from-cte", [atom(cte)]));
    sub.push(list("reduces", [atom(reduces)]));
    if let Some(increment) = increment {
        sub.push(list("increment", [list("filter", [increment])]));
    }
    let driver = input.map_or_else(|| "input".to_string(), |i| i.relation.clone());
    list(
        "split",
        [
            atom(model),
            list("model", sub),
            list(
                "model",
                [
                    atom(model),
                    list("materialized", [atom("table")]),
                    list("reads", [atom(part)]),
                ],
            ),
            list(
                "cost",
                [
                    list("before", [list("full-scan", [atom(&driver)])]),
                    list(
                        "after",
                        [
                            list("delta-scan", [atom(&driver)]),
                            list("full-scan", [atom(part)]),
                        ],
                    ),
                ],
            ),
        ],
    )
}

fn newest(column: &str) -> Sexp {
    list("max-of", [atom("this"), atom(column)])
}

fn newest_is_null(column: &str) -> Sexp {
    list("is-null", [newest(column)])
}

fn greater(input: &DeltaInput, out_column: &str) -> Sexp {
    list(
        "or",
        [
            newest_is_null(out_column),
            list(">", [atom(&input.column), newest(out_column)]),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::super::tests::Facts;
    use super::*;

    fn facts() -> Facts {
        Facts::new("duckdb").yaml(
            "events",
            &[
                ("id", Some("bigint")),
                ("user_id", Some("bigint")),
                ("ts", Some("timestamp")),
                ("amount", Some("decimal(18,2)")),
                ("status", Some("varchar")),
            ],
        )
    }

    const RANKED: &str = "{{ config(materialized='table') }}\nwith by_user as (\n  select user_id, sum(amount) as total, max(ts) as last_ts\n  from {{ ref('events') }}\n  where status = 'ok'\n  group by user_id\n)\nselect user_id, total, rank() over (order by total desc) as rnk\nfrom by_user\n";

    fn applied(text: &str, edits: &[TextEdit]) -> String {
        // single-line positions only in these tests' edits: convert by lines
        let starts: Vec<usize> = std::iter::once(0)
            .chain(text.match_indices('\n').map(|(i, _)| i + 1))
            .collect();
        let at = |p: cogitator_lsp::Position| starts[p.line as usize] + p.character as usize;
        let mut out = text.to_string();
        let mut sorted: Vec<&TextEdit> = edits.iter().collect();
        sorted.sort_by_key(|e| std::cmp::Reverse(at(e.range.start)));
        for e in sorted {
            out.replace_range(at(e.range.start)..at(e.range.end), &e.new_text);
        }
        out
    }

    #[test]
    fn a_ranked_rollup_splits_into_an_incremental_part_and_a_table() {
        let facts = facts();
        let plan = split_plan(&facts.input(RANKED, Some("orders_ranked"))).expect("a plan");
        let part = plan.part.as_ref().expect("a split");
        assert_eq!(part.name, "orders_ranked__by_user");
        insta::assert_snapshot!("split_sexp", plan.sexp);
        insta::assert_snapshot!("split_part", part.text);
        insta::assert_snapshot!("split_outer", applied(RANKED, &plan.edits));
    }

    #[test]
    fn a_window_straight_over_the_input_is_refused() {
        let facts = facts();
        let text = "{{ config(materialized='table') }}\nselect id, ts, rank() over (order by ts) as r from {{ ref('events') }}\n";
        let plan = split_plan(&facts.input(text, Some("events_ranked"))).expect("a plan");
        assert!(plan.part.is_none());
        assert!(plan.sexp.contains("(refused no-reduction"), "{}", plan.sexp);
    }

    #[test]
    fn a_model_that_is_incremental_already_has_nothing_to_split() {
        let facts = facts();
        let text = "{{ config(materialized='table') }}\nselect id, ts from {{ ref('events') }}\n";
        assert!(split_plan(&facts.input(text, Some("plain"))).is_none());
    }
}
