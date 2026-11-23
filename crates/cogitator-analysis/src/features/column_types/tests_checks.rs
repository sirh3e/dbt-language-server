//! The checks over `testdata/type_check/models/schema.yml`.

use cogitator_lsp::{diagnostic_severity, FixKind, Position, Range};

use super::tests::{fixture_root, project};
use super::*;

fn schema_yml() -> String {
    std::fs::read_to_string(fixture_root().join("models/schema.yml")).expect("fixture yaml")
}

/// (code, severity, line, message start) of every diagnostic.
fn mismatches() -> Vec<(String, i32, u32, String)> {
    let (ctx, snapshot) = project();
    let types = |m: &str| column_types(&ctx, &snapshot, m);
    let mut out: Vec<_> = type_mismatch_diagnostics(&ctx, &schema_yml(), &types)
        .into_iter()
        .map(|d| {
            (
                d.code.clone(),
                d.severity,
                d.range.start.line,
                d.message.clone(),
            )
        })
        .collect();
    out.sort_by_key(|d| d.2);
    out
}

#[test]
fn only_proven_disagreements_are_reported() {
    let found = mismatches();
    let lines: Vec<(u32, i32)> = found.iter().map(|d| (d.2, d.1)).collect();
    assert_eq!(
        lines,
        vec![
            // stg_orders.order_id: bigint declared varchar, contract → error
            (8, diagnostic_severity::ERROR),
            // order_stats.order_count: count() declared text → warning
            (21, diagnostic_severity::WARNING),
            // unioned.label: text declared integer → warning
            (28, diagnostic_severity::WARNING),
        ],
        "{found:#?}"
    );
    assert!(found.iter().all(|d| d.0 == MISMATCH));
    assert!(found[0].3.contains("dbt build fails"), "{}", found[0].3);
    assert!(found[1].3.contains("count()"), "{}", found[1].3);
}

#[test]
fn the_mismatch_offers_the_spelled_type() {
    let (ctx, snapshot) = project();
    let types = |m: &str| column_types(&ctx, &snapshot, m);
    let diags = type_mismatch_diagnostics(&ctx, &schema_yml(), &types);
    let first = diags
        .iter()
        .find(|d| d.range.start.line == 8)
        .expect("order_id");
    let Some(FixKind::MultiEdit { fixes }) = first.data.as_ref().map(|d| &d.fix) else {
        panic!("a fix");
    };
    assert_eq!(fixes[0].title, "Set data_type to bigint");
    assert_eq!(fixes[0].edits[0].new_text, "bigint");
    assert_eq!(first.range.start.character, 19);
    assert_eq!(first.range.end.character, 26);
    // count() has no exact spelling: no fix rather than a guess
    let count = diags
        .iter()
        .find(|d| d.range.start.line == 21)
        .expect("count");
    assert!(count.data.is_none());
}

#[test]
fn fill_writes_the_missing_contract_types() {
    let (ctx, snapshot) = project();
    let types = |m: &str| column_types(&ctx, &snapshot, m);
    let at = Range::zero_width(Position {
        line: 2,
        character: 4,
    });
    let action = fill_data_types_action(&ctx, &schema_yml(), "file:///s.yml", at, &types)
        .expect("stg_orders has a contract and an untyped column");
    let edits = match &action.edit.as_ref().expect("edit").document_changes[0] {
        cogitator_lsp::DocumentChange::Edit(e) => &e.edits,
        other => panic!("{other:?}"),
    };
    assert_eq!(edits.len(), 1, "{edits:?}");
    assert_eq!(edits[0].range.start.line, 14);
    assert_eq!(edits[0].new_text, "        data_type: date\n");
    // no contract, nothing offered
    let stats = Range::zero_width(Position {
        line: 18,
        character: 4,
    });
    assert!(fill_data_types_action(&ctx, &schema_yml(), "file:///s.yml", stats, &types).is_none());
}

#[test]
fn impossible_unit_test_values_are_hinted() {
    let (ctx, snapshot) = project();
    let types = |m: &str| column_types(&ctx, &snapshot, m);
    let diags = unit_test_value_diagnostics(&schema_yml(), &ctx.dialect, &types);
    let mut at: Vec<(u32, u32)> = diags
        .iter()
        .map(|d| (d.range.start.line, d.range.start.character))
        .collect();
    at.sort();
    // given order_id: 'abc' (bigint), expect order_count: 'many' (count)
    assert_eq!(at, vec![(35, 23), (39, 37)], "{diags:#?}");
    assert!(diags
        .iter()
        .all(|d| d.code == VALUE_TYPE && d.severity == diagnostic_severity::HINT));
}

#[test]
fn yaml_hover_shows_the_inferred_type() {
    let (ctx, snapshot) = project();
    let types = |m: &str| column_types(&ctx, &snapshot, m);
    let text = schema_yml();
    let hover = yaml_column_hover(
        &ctx,
        &text,
        Position {
            line: 7,
            character: 16,
        },
        &types,
    )
    .expect("on order_id");
    assert!(
        hover.starts_with("Type: `bigint` — an explicit cast"),
        "{hover}"
    );
    assert!(hover.contains("Declared `varchar`"), "{hover}");
    let declared = yaml_column_hover(
        &ctx,
        &text,
        Position {
            line: 22,
            character: 16,
        },
        &types,
    )
    .expect("on code");
    assert!(declared.contains("nothing enforces"), "{declared}");
}

#[rstest::rstest]
#[case("'abc'", cogitator_ir::SqlType::Int { bits: None }, false)]
#[case("'12'", cogitator_ir::SqlType::Int { bits: None }, true)]
#[case("1.5", cogitator_ir::SqlType::Int { bits: None }, true)]
#[case("true", cogitator_ir::SqlType::Int { bits: None }, false)]
#[case("'1.5e3'", cogitator_ir::SqlType::Float { bits: None }, true)]
#[case("'maybe'", cogitator_ir::SqlType::Bool, false)]
#[case("'yes'", cogitator_ir::SqlType::Bool, true)]
#[case("2", cogitator_ir::SqlType::Bool, false)]
#[case("'2024-01-01'", cogitator_ir::SqlType::Date, true)]
#[case("true", cogitator_ir::SqlType::Date, false)]
#[case("null", cogitator_ir::SqlType::Int { bits: None }, true)]
#[case("[1, 2]", cogitator_ir::SqlType::Int { bits: None }, false)]
#[case("{a: 1}", cogitator_ir::SqlType::Json, true)]
#[case("42", cogitator_ir::SqlType::Text, true)]
fn row_values(#[case] yaml: &str, #[case] ty: cogitator_ir::SqlType, #[case] want: bool) {
    use saphyr::LoadableYamlNode;
    let docs = saphyr::MarkedYaml::load_from_str(yaml).expect("yaml");
    assert_eq!(super::unit_values::fits(&docs[0].data, &ty), want, "{yaml}");
}
