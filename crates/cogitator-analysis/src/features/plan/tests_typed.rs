//! The type checks and `join.fanout` over `testdata/type_check`, through
//! the real schema job and planner, as the default settings report them.

use cogitator_lsp::{diagnostic_severity, Diagnostic, DiagnosticSubject};

use super::diagnostics::plan_diagnostics;
use super::diagnostics_config::PerfSettings;
use super::suppress::scan;
use super::{query_plan, PlanInput};
use crate::artifacts::ArtifactSet;
use crate::features::column_types::tests::project;

fn diagnostics(model: &str) -> Vec<Diagnostic> {
    let (ctx, snapshot) = project();
    let path = ctx.model_detail_map[model].uri.clone();
    let text = std::fs::read_to_string(path).expect("model file");
    let artifacts = ArtifactSet::default();
    let input = PlanInput::from_context(&ctx, &text, Some(model), &artifacts, snapshot.revision)
        .with_project(Some(&snapshot));
    let plan = query_plan(&input);
    plan_diagnostics(
        &text,
        &plan,
        &scan(&text, &ctx.dialect),
        &PerfSettings::default(),
    )
}

fn codes(found: &[Diagnostic]) -> Vec<(&str, i32)> {
    let mut out: Vec<(&str, i32)> = found
        .iter()
        .map(|d| (d.code.as_str(), d.severity))
        .collect();
    out.sort_unstable();
    out
}

#[test]
fn text_against_a_proven_date_is_a_warning() {
    assert_eq!(
        codes(&diagnostics("late_events")),
        vec![("type-text-temporal-compare", diagnostic_severity::WARNING)]
    );
}

#[test]
fn a_sum_over_text_is_a_warning() {
    assert_eq!(
        codes(&diagnostics("channel_totals")),
        vec![("type-aggregate-text", diagnostic_severity::WARNING)]
    );
}

#[test]
fn a_join_on_an_untested_key_and_a_declared_type_mismatch() {
    let found = diagnostics("order_codes");
    // the source's integer is only declared in YAML: an Info, not a Warning
    assert_eq!(
        codes(&found),
        vec![
            ("join.fanout", diagnostic_severity::HINT),
            ("type-join-key-mismatch", diagnostic_severity::INFO),
        ]
    );
    let fanout = found
        .iter()
        .find(|d| d.code == "join.fanout")
        .expect("fanout");
    assert_eq!(
        fanout.message,
        "this join may duplicate rows of stg_orders: code is not known to be unique in shop.legacy [heuristic]"
    );
    let data = fanout.data.as_ref().expect("data");
    assert_eq!(
        data.subject,
        Some(DiagnosticSubject::UniqueKey {
            relation: "legacy".into(),
            source: Some("shop".into()),
            column: "code".into(),
        })
    );
}

#[test]
fn models_without_such_shapes_stay_quiet() {
    for model in ["stg_orders", "unioned", "legacy_codes"] {
        assert!(
            diagnostics(model).is_empty(),
            "{model}: {:?}",
            diagnostics(model)
        );
    }
}
