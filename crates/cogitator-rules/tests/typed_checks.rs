//! `type.text_temporal_compare`, `type.aggregate_text` and the proven /
//! declared severity split of `join.key_type_mismatch`.

mod logical_support;

use cogitator_rules::{Finding, Severity};
use logical_support::{all_needs, col, only, run_rule, schemas, Col, Options, Type};

fn run(sql: &str, a: &[Col], b: &[Col], rule: &str) -> Vec<Finding> {
    run_rule(
        sql,
        schemas(&[("a", a), ("b", b)]),
        all_needs(),
        Options::default(),
        rule,
    )
}

const COMPARE: &str = "type.text_temporal_compare";
const AGG: &str = "type.aggregate_text";

#[test]
fn text_compared_with_a_date_warns_when_both_types_are_proven() {
    let a = [
        col("id"),
        col("code").typed(Type::Text),
        col("day").typed(Type::Date),
    ];
    let sql = "select id from {{ ref('a') }} where code > day";
    let found = only(run(sql, &a, &a, COMPARE));
    assert_eq!(found.severity, Severity::Warning);
    assert!(
        found.message.contains("text with a date"),
        "{}",
        found.message
    );
}

#[test]
fn a_declared_type_lowers_it_to_info() {
    let a = [
        col("id"),
        col("code").typed(Type::Text).declared(),
        col("day").typed(Type::Timestamp),
    ];
    let sql = "select id from {{ ref('a') }} where code = day";
    assert_eq!(only(run(sql, &a, &a, COMPARE)).severity, Severity::Info);
}

#[test]
fn a_string_literal_against_a_date_is_the_idiom() {
    let a = [col("id"), col("day").typed(Type::Date)];
    let sql = "select id from {{ ref('a') }} where day >= '2024-01-01'";
    assert!(run(sql, &a, &a, COMPARE).is_empty());
}

#[test]
fn join_on_case_and_having_are_checked() {
    let a = [col("id"), col("code").typed(Type::Text)];
    let b = [col("id"), col("day").typed(Type::Date)];
    let on = "select x.id from {{ ref('a') }} as x join {{ ref('b') }} as y on x.code = y.day";
    assert_eq!(run(on, &a, &b, COMPARE).len(), 1);
    let case = "select x.id, case when x.code < y.day then 1 else 0 end as early \
                from {{ ref('a') }} as x join {{ ref('b') }} as y on x.id = y.id";
    assert_eq!(run(case, &a, &b, COMPARE).len(), 1);
}

#[test]
fn same_family_comparisons_are_silent() {
    let a = [
        col("id"),
        col("day").typed(Type::Date),
        col("at").typed(Type::Timestamp),
    ];
    let sql = "select id from {{ ref('a') }} where day < at";
    assert!(run(sql, &a, &a, COMPARE).is_empty());
}

#[test]
fn sum_over_text_is_reported_and_over_numbers_is_not() {
    let a = [
        col("id"),
        col("amount").typed(Type::Text),
        col("n").typed(Type::Decimal),
    ];
    let sql = "select id, sum(amount) as total, avg(n) as mean from {{ ref('a') }} group by id";
    let found = only(run(sql, &a, &a, AGG));
    assert_eq!(found.severity, Severity::Warning);
    assert!(found.message.starts_with("sum()"), "{}", found.message);
    let counted = "select id, count(amount) as n from {{ ref('a') }} group by id";
    assert!(run(counted, &a, &a, AGG).is_empty());
}

#[test]
fn key_mismatch_on_a_declared_type_is_info() {
    let a = [col("id"), col("key").typed(Type::Int)];
    let b = [col("id"), col("key").typed(Type::Text).declared()];
    let sql = "select x.id from {{ ref('a') }} as x join {{ ref('b') }} as y on x.key = y.key";
    let found = only(run(sql, &a, &b, "join.key_type_mismatch"));
    assert_eq!(found.severity, Severity::Info);
}
