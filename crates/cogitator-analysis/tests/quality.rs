//! Quality lenses and the unit-test scaffold over the jaffle fixture:
//! coverage counts, "Run this test" selectors, and a `unit_tests:` entry.

use std::path::PathBuf;

use cogitator_analysis::features::quality::{quality_lenses, yaml_tests};
use cogitator_analysis::features::unit_test_scaffold_action;
use cogitator_analysis::{build_context, DbtContext};
use cogitator_lsp::{fs_path_to_uri, DocumentChange};
use pretty_assertions::assert_eq;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/jaffle_shop_duckdb")
}

fn ctx() -> DbtContext {
    build_context(root().to_str().unwrap())
}

fn titles(ctx: &DbtContext, path: &str) -> Vec<String> {
    let file = root().join(path);
    let text = std::fs::read_to_string(&file).unwrap();
    quality_lenses(ctx, &fs_path_to_uri(&file), Some(&text), None)
        .into_iter()
        .map(|l| l.command.title)
        .collect()
}

#[test]
fn sql_lens_counts_documented_and_tested_columns() {
    let ctx = ctx();
    assert_eq!(
        titles(&ctx, "models/customers.sql"),
        vec!["7/7 columns documented · 1/7 tested".to_string()]
    );
}

#[test]
fn yaml_lenses_cover_entries_columns_and_tests() {
    let ctx = ctx();
    let titles = titles(&ctx, "models/staging/schema.yml");
    // stg_customers: one undocumented, tested column, then its two tests
    assert_eq!(
        &titles[..4],
        &[
            "0/1 columns documented · 1/1 tested · no description",
            "no description",
            "Run this test",
            "Run this test",
        ]
    );
}

#[test]
fn generic_tests_get_dbt_names() {
    let text = std::fs::read_to_string(root().join("models/schema.yml")).unwrap();
    let selectors: Vec<String> = yaml_tests(&text).into_iter().map(|t| t.selector).collect();
    for expected in [
        "unique_customers_customer_id",
        "not_null_orders_order_id",
        "relationships_orders_customer_id__customer_id__ref_customers_",
        "accepted_values_orders_status__placed__shipped__completed__return_pending__returned",
    ] {
        assert!(
            selectors.iter().any(|s| s == expected),
            "{expected}: {selectors:?}"
        );
    }
}

#[test]
fn singular_test_file_runs_itself() {
    let ctx = ctx();
    let file = root().join("tests/assert_positive_order_amount.sql");
    let lenses = quality_lenses(&ctx, &fs_path_to_uri(&file), None, None);
    assert_eq!(lenses.len(), 1);
    assert_eq!(lenses[0].command.command, "dbt.runSelection");
    assert_eq!(
        lenses[0].command.arguments[0]["selector"],
        "assert_positive_order_amount"
    );
    assert_eq!(lenses[0].command.arguments[0]["kind"], "test");
}

#[test]
fn scaffold_reads_the_inputs_documented_columns() {
    let ctx = ctx();
    let details = ctx.model_detail_map.get("orders").unwrap();
    let sql = std::fs::read_to_string(&details.uri).unwrap();
    let schema = details.schema_uri.clone().unwrap();
    let yaml = std::fs::read_to_string(&schema).unwrap();
    let action = unit_test_scaffold_action(
        &ctx,
        "orders",
        details,
        &sql,
        &yaml,
        &fs_path_to_uri(&schema),
        None,
    )
    .expect("an action");
    let edit = action.edit.unwrap();
    let DocumentChange::Edit(edit) = &edit.document_changes[0] else {
        panic!("an edit");
    };
    let text = &edit.edits[0].new_text;
    assert!(
        text.contains("unit_tests:\n  - name: test_orders\n    model: orders\n"),
        "{text}"
    );
    assert!(
        text.contains("- input: ref('stg_orders')\n        rows:\n          - {order_id: null, status: null}\n"),
        "{text}"
    );
    assert!(
        text.contains("- input: ref('stg_payments')\n        rows:\n          - {payment_id: null, payment_method: null}\n"),
        "{text}"
    );
    assert!(
        text.contains("expect:\n      rows:\n        - {order_id: null, customer_id: null"),
        "{text}"
    );
}
