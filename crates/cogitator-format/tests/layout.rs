//! The formatter end to end, on models whose layout is written out here:
//! what each rule makes of them, that the Jinja comes back, that a second
//! run changes nothing, and what is declined.

use cogitator_format::{format, Case, Commas, Config, Declined};
use pretty_assertions::assert_eq;
use rstest::rstest;

fn duckdb() -> Config {
    Config {
        dialect: "duckdb".into(),
        ..Config::default()
    }
}

/// Formats, and holds the result to being formatted already.
fn fmt(source: &str, config: &Config) -> String {
    let once = format(source, config).unwrap_or_else(|e| panic!("declined: {e}\n{source}"));
    let twice = format(&once, config).expect("formats its own output");
    assert_eq!(once, twice, "a second run changes nothing");
    once
}

#[test]
fn a_model_with_ctes_joins_and_jinja() {
    let source = "{{ config(materialized='table') }}\n\nwith src as (select * from {{ref( 'stg_orders' )}}), agg as (SELECT customer_id, SUM(amount) AS total FROM src WHERE status != 'returned' GROUP BY customer_id) select a.customer_id, a.total, c.name from agg a left join {{ ref('customers') }} c on a.customer_id = c.id\n";
    let expected = "\
{{ config(materialized='table') }}

with src as (
    select *
    from {{ ref('stg_orders') }}
),

agg as (
    select
        customer_id,
        sum(amount) as total
    from src
    where status != 'returned'
    group by customer_id
)

select
    a.customer_id,
    a.total,
    c.name
from agg a
left join {{ ref('customers') }} c on a.customer_id = c.id
";
    assert_eq!(fmt(source, &duckdb()), expected);
}

#[test]
fn jinja_blocks_indent_the_lines_inside_them() {
    let source = "select a from t\n{% if is_incremental() %}\nwhere a > (select max(a) from {{ this }})\n{% endif %}\n";
    let expected = "\
select a
from t
{% if is_incremental() %}
    where a > (
        select max(a)
        from {{ this }}
    )
{% endif %}
";
    assert_eq!(fmt(source, &duckdb()), expected);
}

#[test]
fn long_expressions_break_before_their_operators() {
    let source = "select error('the snapshot ' || document_url || ' was asked in ' || currency || ' and answers ' || answered) as e from t\n";
    let expected = "\
select
    error(
        'the snapshot '
            || document_url
            || ' was asked in '
            || currency
            || ' and answers '
            || answered
    ) as e
from t
";
    assert_eq!(fmt(source, &duckdb()), expected);
}

#[test]
fn a_case_breaks_into_its_whens() {
    let source = "select case when a is null or a = b then 'one thing' when c > 100 then 'another' else 'the rest of them' end as kind from t\n";
    let expected = "\
select
    case
        when a is null or a = b then 'one thing'
        when c > 100 then 'another'
        else 'the rest of them'
    end as kind
from t
";
    assert_eq!(fmt(source, &duckdb()), expected);
}

#[rstest]
#[case::upper_keywords(
    Config { case: cogitator_format::CaseRules { keywords: Case::Upper, functions: Case::Upper, ..Default::default() }, ..duckdb() },
    "SELECT\n    a,\n    COUNT(*) AS n\nFROM t\nGROUP BY a\n"
)]
#[case::leading_commas(
    Config { layout: cogitator_format::config::LayoutRules { commas: Commas::Leading, ..Default::default() }, ..duckdb() },
    "select\n    a\n    , count(*) as n\nfrom t\ngroup by a\n"
)]
#[case::tabs(
    Config { indent_unit: cogitator_format::IndentUnit::Tab, ..duckdb() },
    "select\n\ta,\n\tcount(*) as n\nfrom t\ngroup by a\n"
)]
fn the_rules_change_the_layout(#[case] config: Config, #[case] expected: &str) {
    assert_eq!(fmt("select a, count(*) as n from t group by a", &config), expected);
}

#[test]
fn operators_lexed_as_two_tokens_stay_one() {
    let out = fmt("select a||b as c, x::decimal(18,3) as d from t where a!=b\n", &duckdb());
    assert!(out.contains("a || b as c"), "{out}");
    assert!(out.contains("x::decimal(18, 3) as d"), "{out}");
    assert!(out.contains("a != b"), "{out}");
}

#[test]
fn a_macro_whose_body_is_not_sql_is_kept_as_written() {
    let source = "{% macro key_pattern() -%}\n    /[a-z0-9_]+/[A-Z]{3}\\.json$\n{%- endmacro %}\n";
    assert_eq!(fmt(source, &duckdb()), source);
}

#[test]
fn crlf_endings_are_kept() {
    let out = fmt("select a,b from t\r\n", &duckdb());
    assert_eq!(out, "select\r\n    a,\r\n    b\r\nfrom t\r\n");
}

#[test]
fn broken_jinja_and_unknown_dialects_are_declined() {
    assert!(matches!(format("select {{ x", &duckdb()), Err(Declined::Jinja(_))));
    let config = Config {
        dialect: "nope".into(),
        ..Config::default()
    };
    assert!(matches!(format("select 1", &config), Err(Declined::Dialect(_))));
}
