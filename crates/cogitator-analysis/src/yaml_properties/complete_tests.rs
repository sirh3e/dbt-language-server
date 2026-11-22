use super::*;
use crate::yaml_properties::builtin_tests;
use rstest::rstest;

fn labels_at(marked: &str) -> Vec<(String, String)> {
    let offset = marked.find('|').expect("cursor mark");
    let text = marked.replace('|', "");
    let before = &marked[..offset];
    let pos = Position {
        line: before.matches('\n').count() as u32,
        character: before.rsplit('\n').next().unwrap_or("").chars().count() as u32,
    };
    let mut tests = builtin_tests();
    tests.push(TestDefinition::new(
        "at_least_one",
        Some("dbt_utils"),
        &["model", "column_name", "group_by"],
    ));
    tests.push(TestDefinition::new(
        "positive",
        None,
        &["model", "column_name"],
    ));
    let output = vec!["customer_id".to_string(), "first_name".to_string()];
    let declared = vec!["customer_id".to_string()];
    let inputs = PropertiesInputs {
        model: Some("customers"),
        output_columns: &output,
        declared_columns: &declared,
        tests: &tests,
    };
    properties_completion(&text, pos, &inputs)
        .into_iter()
        .map(|i| (i.label, i.insert_text))
        .collect()
}

const MODEL: &str = "models:\n  - name: customers\n";

#[rstest]
#[case::model_keys(&format!("{MODEL}    |"), "columns", "columns:")]
#[case::scalar_key(&format!("{MODEL}    desc|"), "description", "description: ")]
#[case::config_keys(&format!("{MODEL}    config:\n      |"), "materialized", "materialized: ")]
#[case::column_keys(&format!("{MODEL}    columns:\n      - name: a\n        |"), "data_type", "data_type: ")]
#[case::column_name(&format!("{MODEL}    columns:\n      - name: |"), "first_name", "first_name")]
#[case::column_item(&format!("{MODEL}    columns:\n      - |"), "first_name", "name: first_name")]
#[case::column_dash(&format!("{MODEL}    columns:\n      |"), "first_name", "- name: first_name")]
#[case::test_names(&format!("{MODEL}    data_tests:\n      - |"), "not_null", "not_null")]
#[case::test_with_args(&format!("{MODEL}    data_tests:\n      - |"), "accepted_values", "accepted_values:")]
#[case::package_test(&format!("{MODEL}    data_tests:\n      - |"), "dbt_utils.at_least_one", "dbt_utils.at_least_one:")]
#[case::project_test(&format!("{MODEL}    data_tests:\n      - |"), "positive", "positive")]
#[case::test_dash(&format!("{MODEL}    data_tests:\n      |"), "unique", "- unique")]
#[case::builtin_args(
    &format!("{MODEL}    columns:\n      - name: a\n        data_tests:\n          - accepted_values:\n              |"),
    "values",
    "values:"
)]
#[case::relationships_args(
    &format!("{MODEL}    data_tests:\n      - relationships:\n          |"),
    "field",
    "field: "
)]
#[case::custom_args(
    &format!("{MODEL}    data_tests:\n      - dbt_utils.at_least_one:\n          |"),
    "group_by",
    "group_by: "
)]
#[case::custom_arguments_block(
    &format!("{MODEL}    data_tests:\n      - dbt_utils.at_least_one:\n          arguments:\n            |"),
    "group_by",
    "group_by: "
)]
#[case::enum_value(&format!("{MODEL}    access: |"), "protected", "protected")]
#[case::bool_value(
    &format!("{MODEL}    data_tests:\n      - accepted_values:\n          quote: |"),
    "false",
    "false"
)]
#[case::sources(&"sources:\n  - name: s\n    tables:\n      - name: t\n        |".to_string(), "identifier", "identifier: ")]
fn completion_offers(#[case] marked: &str, #[case] label: &str, #[case] insert: &str) {
    let got = labels_at(marked);
    assert!(
        got.iter().any(|(l, i)| l == label && i == insert),
        "{label} → {insert} missing from {got:?}"
    );
}

#[rstest]
#[case::no_model_columns_in_sources(&"sources:\n  - name: s\n    tables:\n      - name: |".to_string(), "first_name")]
#[case::no_columns_as_tests(&format!("{MODEL}    data_tests:\n      - |"), "first_name")]
#[case::model_column_param_hidden(
    &format!("{MODEL}    data_tests:\n      - dbt_utils.at_least_one:\n          |"),
    "column_name"
)]
fn completion_leaves_out(#[case] marked: &str, #[case] label: &str) {
    let got = labels_at(marked);
    assert!(!got.iter().any(|(l, _)| l == label), "{label} in {got:?}");
}

#[test]
fn undeclared_columns_sort_first() {
    let text = format!("{MODEL}    columns:\n      - name: ");
    let pos = Position {
        line: 3,
        character: 14,
    };
    let output = vec!["customer_id".to_string(), "first_name".to_string()];
    let declared = vec!["customer_id".to_string()];
    let inputs = PropertiesInputs {
        model: Some("customers"),
        output_columns: &output,
        declared_columns: &declared,
        tests: &[],
    };
    let items = properties_completion(&text, pos, &inputs);
    let sort = |l: &str| {
        items
            .iter()
            .find(|i| i.label == l)
            .map(|i| i.sort_text.clone())
    };
    assert!(sort("first_name") < sort("customer_id"));
}
