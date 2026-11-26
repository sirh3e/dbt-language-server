use std::collections::HashMap;

use pretty_assertions::assert_eq;
use rstest::rstest;

use super::*;

fn inputs(macros: &[&str]) -> PreviewInputs {
    PreviewInputs {
        macro_sources: macros.iter().map(|m| m.to_string()).collect(),
        model_relations: HashMap::from([
            ("orders".to_string(), "shop.main.orders".to_string()),
            ("customers".to_string(), "shop.main.customers".to_string()),
        ]),
        source_relations: HashMap::new(),
        vars: HashMap::new(),
        target_name: "dev".to_string(),
        is_incremental: false,
        this: None,
        env: HashMap::new(),
    }
}

const YAML: &str = "version: 2
models:
  - name: orders
    data_tests:
      - dbt_utils.expression_is_true:
          arguments:
            expression: \"amount >= 0\"
    columns:
      - name: order_id
        data_tests:
          - unique
          - not_null:
              config:
                severity: warn
                where: \"status != 'test'\"
      - name: status
        data_tests:
          - accepted_values:
              values: ['placed', 'shipped']
      - name: customer_id
        tests:
          - relationships:
              to: ref('customers')
              field: id
unit_tests:
  - name: orders_add_up
    model: orders
";

fn rendered(macros: &[&str]) -> Vec<RenderedTest> {
    let inputs = inputs(macros);
    let renderer = TestRenderer::new(&inputs, "duckdb");
    yaml_tests(YAML, "orders", "file:///p/models/orders.yml", &renderer)
}

#[test]
fn every_test_is_found_with_its_arguments() {
    let labels: Vec<_> = rendered(&[])
        .into_iter()
        .map(|t| (t.label, t.kind, t.severity))
        .collect();
    assert_eq!(
        labels,
        vec![
            (
                "dbt_utils.expression_is_true".to_string(),
                TestKind::Generic,
                "error".to_string()
            ),
            (
                "unique on order_id".to_string(),
                TestKind::Generic,
                "error".to_string()
            ),
            (
                "not_null on order_id".to_string(),
                TestKind::Generic,
                "warn".to_string()
            ),
            (
                "accepted_values on status".to_string(),
                TestKind::Generic,
                "error".to_string()
            ),
            (
                "relationships on customer_id".to_string(),
                TestKind::Generic,
                "error".to_string()
            ),
            (
                "unit test orders_add_up".to_string(),
                TestKind::Unit,
                "error".to_string()
            ),
        ]
    );
}

#[rstest]
#[case::unique(
    "unique on order_id",
    "select\n    order_id as unique_field,\n    count(*) as n_records\n\nfrom shop.main.orders\n\
     where order_id is not null\ngroup by order_id\nhaving count(*) > 1\n"
)]
#[case::not_null_with_where(
    "not_null on order_id",
    "select order_id\nfrom (select * from shop.main.orders where status != 'test') dbt_subquery\n\
     where order_id is null\n"
)]
#[case::accepted_values(
    "accepted_values on status",
    "with all_values as (\n\n    select\n        status as value_field,\n        count(*) as n_records\n\n\
     \x20   from shop.main.orders\n    group by status\n\n)\n\nselect *\nfrom all_values\n\
     where value_field not in ('placed', 'shipped'\n)\n"
)]
#[case::relationships(
    "relationships on customer_id",
    "with child as (\n    select customer_id as from_field\n    from shop.main.orders\n    \
     where customer_id is not null\n),\n\nparent as (\n    select id as to_field\n    \
     from shop.main.customers\n)\n\nselect\n    from_field\n\nfrom child\nleft join parent\n    \
     on child.from_field = parent.to_field\n\nwhere parent.to_field is null\n"
)]
fn the_built_in_tests_render_as_dbt_writes_them(#[case] label: &str, #[case] sql: &str) {
    let test = rendered(&[])
        .into_iter()
        .find(|t| t.label == label)
        .unwrap();
    assert_eq!(test.defined_by.as_deref(), Some("dbt (built-in)"));
    assert_eq!(test.error, None);
    assert_eq!(test.sql.as_deref(), Some(sql));
}

#[test]
fn a_package_test_is_rendered_by_its_default_implementation() {
    let package = "{% test expression_is_true(model, expression) %}\
                   {{ return(adapter.dispatch('test_expression_is_true', 'dbt_utils')(model, expression)) }}\
                   {% endtest %}\n\
                   {% macro default__test_expression_is_true(model, expression) %}\
                   select * from {{ model }} where not ({{ expression }})\
                   {% endmacro %}";
    let test = rendered(&[package])
        .into_iter()
        .find(|t| t.name == "dbt_utils.expression_is_true")
        .unwrap();
    assert_eq!(
        test.defined_by.as_deref(),
        Some("default__test_expression_is_true")
    );
    assert_eq!(
        test.sql.as_deref(),
        Some("select * from shop.main.orders where not (amount >= 0)\n")
    );
}

#[test]
fn a_project_test_block_overrides_nothing_but_renders() {
    let block = "{% test is_positive(model, column_name) %}\
                 select * from {{ model }} where {{ column_name }} < 0\
                 {% endtest %}";
    let yaml = "models:\n  - name: orders\n    columns:\n      - name: amount\n        data_tests: [is_positive]\n";
    let inputs = inputs(&[block]);
    let renderer = TestRenderer::new(&inputs, "duckdb");
    let tests = yaml_tests(yaml, "orders", "file:///o.yml", &renderer);
    assert_eq!(tests[0].defined_by.as_deref(), Some("is_positive"));
    assert_eq!(
        tests[0].sql.as_deref(),
        Some("select * from shop.main.orders where amount < 0\n")
    );
}

#[test]
fn an_unknown_test_says_what_was_looked_for() {
    let yaml = "models:\n  - name: orders\n    data_tests: [no_such_test]\n";
    let inputs = inputs(&[]);
    let renderer = TestRenderer::new(&inputs, "duckdb");
    let tests = yaml_tests(yaml, "orders", "file:///o.yml", &renderer);
    assert!(tests[0].sql.is_none());
    assert!(
        tests[0]
            .error
            .as_deref()
            .unwrap()
            .contains("default__test_no_such_test"),
        "{:?}",
        tests[0].error
    );
}

#[test]
fn the_executed_query_counts_the_failures() {
    let mut config = Map::new();
    config.insert("limit".into(), Json::from(10));
    config.insert("warn_if".into(), Json::from(">10"));
    assert_eq!(
        executed_query("select 1\n", &config),
        "select\n    count(*) as failures,\n    count(*) >10 as should_warn,\n    \
         count(*) != 0 as should_error\nfrom (\n    select 1\n    limit 10\n) dbt_internal_test\n"
    );
}

#[test]
fn a_singular_test_is_rendered_with_its_refs() {
    let inputs = inputs(&[]);
    let renderer = TestRenderer::new(&inputs, "duckdb");
    let file = SingularFile {
        path: "/p/tests/no_negative_orders.sql".into(),
        text: "{{ config(severity='warn') }}\nselect * from {{ ref('orders') }} where amount < 0"
            .into(),
    };
    let test = renderer.singular("orders", &file);
    assert_eq!(test.label, "singular test no_negative_orders");
    assert_eq!(test.severity, "warn");
    assert_eq!(
        test.sql.as_deref(),
        Some("select * from shop.main.orders where amount < 0\n")
    );
}
