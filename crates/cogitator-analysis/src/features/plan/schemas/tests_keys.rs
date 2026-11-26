//! Uniqueness and type evidence beyond column tests: constraints,
//! combination tests, an incremental `unique_key`, and proven types.

use super::tests::{run, Files, Project};

fn col(
    snapshot: &super::SchemaSnapshot,
    model: &str,
    name: &str,
) -> cogitator_ir::schema::SchemaCol {
    snapshot
        .schemas
        .model(model)
        .expect("schema")
        .columns
        .iter()
        .find(|c| c.name.as_ref() == name)
        .unwrap_or_else(|| panic!("{model}.{name}"))
        .clone()
}

#[test]
fn constraints_and_single_column_keys_make_columns_unique() {
    let project = Project::new()
        .sql("a", "select 1 as id, 2 as other")
        .sql("b", "select 1 as k, 2 as j")
        .sql(
            "c",
            "{{ config(materialized='incremental', unique_key='order_id') }}\nselect 1 as order_id, 2 as n",
        )
        .sql("d", "select 1 as x, 2 as y")
        .yaml(
            "version: 2\nmodels:\n  - name: a\n    columns:\n      - name: id\n        constraints:\n          - type: primary_key\n  - name: b\n    data_tests:\n      - dbt_utils.unique_combination_of_columns:\n          combination_of_columns: [k]\n  - name: d\n    constraints:\n      - type: unique\n        columns: [x, y]\n",
        );
    let ctx = project.ctx();
    let snapshot = run(&ctx, &Files::default(), &mut Default::default());
    assert!(col(&snapshot, "a", "id").unique);
    assert!(!col(&snapshot, "a", "other").unique);
    assert!(col(&snapshot, "b", "k").unique);
    assert!(col(&snapshot, "c", "order_id").unique);
    // a two-column key proves neither column alone
    assert!(!col(&snapshot, "d", "x").unique);
}

#[test]
fn inferred_types_are_proven_unless_they_rest_on_unenforced_yaml() {
    let project = Project::new()
        .sql("up", "select cast(1 as bigint) as id, raw_code from {{ source('s', 't') }}")
        .sql("down", "select id, raw_code from {{ ref('up') }}")
        .yaml(
            "version: 2\nsources:\n  - name: s\n    tables:\n      - name: t\n        columns:\n          - name: raw_code\n            data_type: varchar\n",
        );
    let ctx = project.ctx();
    let snapshot = run(&ctx, &Files::default(), &mut Default::default());
    assert!(col(&snapshot, "down", "id").proven_type);
    assert!(!col(&snapshot, "down", "raw_code").proven_type);
}
