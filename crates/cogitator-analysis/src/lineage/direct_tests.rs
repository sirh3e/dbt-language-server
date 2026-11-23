//! `ColumnLineage::direct`: which output columns are one upstream column
//! passed through unchanged, the case table documentation inheritance
//! (`features::yaml_sync`) relies on.

use std::collections::HashMap;

use cogitator_dialects::Dialect;
use cogitator_parser::{DocumentParser, JinjaSqlParser};

use super::typed::lineage_over;
use super::typed_tests::{env, plan_input, CLOSED};

/// The direct output columns of `sql`, over a closed `a(id, name, amount)`.
fn direct(sql: &str) -> Vec<String> {
    let schemas = env(&[("a", &["id", "name", "amount"], CLOSED)]);
    let dialect = Dialect::new("duckdb");
    let (models, sources) = (HashMap::new(), HashMap::new());
    let doc = JinjaSqlParser.parse(sql, &dialect).expect("parse");
    let input = plan_input(sql, &dialect, &models, &sources);
    let lineage = lineage_over(&doc, &input, &schemas).expect(sql);
    lineage
        .columns
        .iter()
        .filter(|c| c.direct)
        .map(|c| c.name.clone())
        .collect()
}

#[test]
fn pass_through_columns_and_nothing_else_are_direct() {
    let cases: &[(&str, &[&str])] = &[
        ("select id, name from {{ ref('a') }}", &["id", "name"]),
        ("select a.id from {{ ref('a') }} as a", &["id"]),
        ("select id as id from {{ ref('a') }}", &["id"]),
        ("select * from {{ ref('a') }}", &["id", "name", "amount"]),
        (
            "with s as (select * from {{ ref('a') }}), t as (select id, name from s) select * from t",
            &["id", "name", "amount"][..2],
        ),
        (
            "select * from (select id from {{ ref('a') }}) as d",
            &["id"],
        ),
        ("select * from {{ source('raw', 'x') }}", &[]),
        ("select id from {{ source('raw', 'x') }}", &["id"]),
        // not the same column
        ("select id as key from {{ ref('a') }}", &[]),
        ("select amount + 1 as amount from {{ ref('a') }}", &[]),
        ("select upper(name) as name from {{ ref('a') }}", &[]),
        ("select id, 1 as one from {{ ref('a') }}", &["id"]),
        (
            "with s as (select id as name from {{ ref('a') }}) select name from s",
            &[],
        ),
        (
            "with s(name) as (select id from {{ ref('a') }}) select name from s",
            &[],
        ),
        (
            "select * replace (amount * 2 as amount) from {{ ref('a') }}",
            &["id", "name"],
        ),
        (
            "select id from {{ ref('a') }} union all select id from {{ ref('a') }}",
            &[],
        ),
        ("select id from raw_table", &[]),
    ];
    for (sql, want) in cases {
        assert_eq!(direct(sql), *want, "{sql}");
    }
}

/// Every output column of `sql` over a closed `a(id, name, amount)`.
fn names(sql: &str) -> Vec<String> {
    let schemas = env(&[("a", &["id", "name", "amount"], CLOSED)]);
    let dialect = Dialect::new("duckdb");
    let (models, sources) = (HashMap::new(), HashMap::new());
    let doc = JinjaSqlParser.parse(sql, &dialect).expect("parse");
    let input = plan_input(sql, &dialect, &models, &sources);
    let lineage = lineage_over(&doc, &input, &schemas).expect(sql);
    lineage.columns.iter().map(|c| c.name.clone()).collect()
}

#[test]
fn a_star_over_a_cte_with_a_macro_item_lists_what_is_known() {
    let sql = "with f as (select id, amount + 1 as amount, {{ audit_columns() }} \
               from {{ ref('a') }}) select * from f";
    // the macro's columns stay a `*`, after the known ones
    assert_eq!(names(sql), ["id", "amount", "*"]);
    assert_eq!(direct(sql), ["id"]);
    // and a model reading it still sees its named columns
    let outer = "with f as (select id, {{ audit_columns() }} from {{ ref('a') }}), \
                 g as (select * from f) select id from g";
    assert_eq!(direct(outer), ["id"]);
}
