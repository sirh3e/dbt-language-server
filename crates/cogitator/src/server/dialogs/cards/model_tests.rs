use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

const ROOT: Option<&str> = Some("file:///p");

fn info() -> Value {
    json!({
        "model": "orders",
        "uri": "file:///p/models/orders.sql",
        "description": "One row per order.\n\nWith its payments.",
        "tags": ["mart", "nightly"],
        "upstream": [
            {"name": "stg_orders", "kind": "model", "uri": "file:///p/models/staging/stg_orders.sql"},
            {"name": "raw.payments", "kind": "source", "uri": null}
        ],
        "downstream": [],
        "versions": {"latestVersion": 2, "versions": [
            {"v": 1, "uri": "file:///p/models/orders_v1.sql"}, {"v": 2, "uri": null}
        ]},
        "tests": [
            {"name": "unique", "column": "order_id", "version": null, "unit": false,
             "uri": "file:///p/models/schema.yml",
             "range": {"start": {"line": 11, "character": 8}, "end": {"line": 11, "character": 14}}},
            {"name": "test_totals", "column": null, "version": 2, "unit": true,
             "uri": "file:///p/models/schema.yml",
             "range": {"start": {"line": 30, "character": 4}, "end": {"line": 30, "character": 9}}}
        ],
        "compiledUri": null,
        "coverage": {"columns": 3, "documented": 2, "tested": 1, "described": true},
        "columnTypes": {"model": "orders", "revision": 1, "columns": [
            {"name": "order_id", "dataType": "INTEGER", "description": "INTEGER", "nullable": false, "basis": "proven"},
            {"name": "status", "dataType": null, "description": "a text", "nullable": true, "basis": "declared"}
        ]}
    })
}

#[test]
fn the_model_card() {
    insta::assert_snapshot!(model(&info(), ROOT).unwrap());
}

#[test]
fn a_model_without_inferred_types_says_so() {
    let mut answer = info();
    answer.as_object_mut().unwrap().remove("columnTypes");
    answer["compiledUri"] = json!("file:///p/target/compiled/shop/models/orders.sql");
    let card = model(&answer, ROOT).unwrap();
    assert!(card.contains(super::NOT_INFERRED), "{card}");
    assert!(
        card.contains("- compiled SQL: [target/compiled/shop/models/orders.sql]"),
        "{card}"
    );
}

#[test]
fn the_model_graph_has_only_its_neighbours() {
    let graph = json!({
        "model": "orders", "uri": "file:///p/models/orders.sql",
        "upstream": [{"name": "stg_orders", "kind": "model", "uri": "file:///p/models/staging/stg_orders.sql"}],
        "downstream": [],
    });
    assert_eq!(
        model(&graph, ROOT).unwrap(),
        "# orders\n\n\
         model · [models/orders.sql](file:///p/models/orders.sql)\n\
         \n## Upstream (1)\n\n\
         - [stg_orders](file:///p/models/staging/stg_orders.sql) · model\n\
         \n## Downstream (0)\n\n\
         _nothing depends on it_\n"
    );
}

#[test]
fn a_node_card_of_a_model_is_the_model_card() {
    let mut answer = info();
    answer["kind"] = json!("model");
    assert_eq!(node(&answer, ROOT), model(&info(), ROOT));
}

#[test]
fn a_macros_card() {
    let answer = json!({
        "kind": "macro", "name": "cents_to_dollars", "package": "shop", "description": "",
        "params": [{"name": "column"}, {"name": "scale", "default": "2"}],
        "uri": "file:///p/macros/money.sql",
        "callers": [{"kind": "model", "name": "orders", "uri": "file:///p/models/orders.sql"}],
        "calls": [],
    });
    insta::assert_snapshot!(node(&answer, ROOT).unwrap());
}

#[test]
fn a_generic_tests_card() {
    let answer = json!({
        "kind": "generic_test", "name": "not_negative", "package": "shop",
        "params": ["model", "column_name"],
        "uri": "file:///p/tests/generic/not_negative.sql",
        "appliedTo": [{"model": "orders", "column": "amount", "uri": "file:///p/models/schema.yml",
                       "range": {"start": {"line": 4, "character": 0}, "end": {"line": 4, "character": 1}}}],
    });
    let card = node(&answer, ROOT).unwrap();
    assert!(
        card.contains("`{% test not_negative(model, column_name) %}`"),
        "{card}"
    );
    assert!(card.contains("## Applied to (1)"), "{card}");
    assert!(
        card.contains("- [orders.amount](file:///p/models/schema.yml) — line 5"),
        "{card}"
    );
}

#[test]
fn a_singular_tests_card() {
    let answer = json!({
        "kind": "test", "name": "assert_positive", "uri": "file:///p/tests/assert_positive.sql",
        "description": null,
        "refs": [{"kind": "model", "name": "orders", "uri": "file:///p/models/orders.sql"}],
        "calls": [], "upstream": [], "downstream": [], "compiledUri": null,
    });
    let card = node(&answer, ROOT).unwrap();
    assert!(card.contains("## Tests (1)"), "{card}");
    assert!(card.contains("_not yet (dbt compile)_"), "{card}");
    assert!(!card.contains("Downstream"), "{card}");
}

#[test]
fn a_test_is_labelled_by_its_column_and_version() {
    let cases = [
        (json!({"name": "unique"}), "unique"),
        (json!({"name": "unique", "column": "id"}), "unique on id"),
        (json!({"name": "t", "column": null, "version": 3}), "t (v3)"),
    ];
    for (t, label) in cases {
        assert_eq!(test_label(&t), label, "{t}");
    }
}
