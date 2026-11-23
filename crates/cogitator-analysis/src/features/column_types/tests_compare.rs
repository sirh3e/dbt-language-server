//! Case tables: dialect spellings through `parse_sql_type`, compared and
//! rendered back.

use cogitator_dialects::Dialect;
use cogitator_ir::types::{SqlType, Tz};
use cogitator_sql_front::parse_sql_type;
use rstest::rstest;

use super::compare::{compare, describe, render, Verdict};

fn ty(raw: &str, dialect: &str) -> SqlType {
    parse_sql_type(raw, &Dialect::new(dialect))
}

#[rstest]
// spellings of one type agree
#[case("duckdb", "int", "integer", Verdict::Agree)]
#[case("duckdb", "int4", "integer", Verdict::Agree)]
#[case("postgres", "bigint", "int8", Verdict::Agree)]
#[case("duckdb", "varchar", "text", Verdict::Agree)]
#[case("bigquery", "string", "string", Verdict::Agree)]
#[case("snowflake", "varchar(20)", "text", Verdict::Agree)]
#[case("postgres", "numeric(10,2)", "decimal(10,2)", Verdict::Agree)]
#[case("postgres", "timestamptz", "timestamp with time zone", Verdict::Agree)]
// widths and precisions stay silent
#[case("duckdb", "integer", "bigint", Verdict::Agree)]
#[case("duckdb", "numeric(10,2)", "numeric(18,4)", Verdict::Agree)]
#[case("duckdb", "integer", "numeric(10,2)", Verdict::Unknown)]
#[case("duckdb", "double", "integer", Verdict::Unknown)]
// different families disagree
#[case("duckdb", "integer", "varchar", Verdict::Disagree)]
#[case("bigquery", "int64", "string", Verdict::Disagree)]
#[case("postgres", "date", "timestamp", Verdict::Disagree)]
#[case("duckdb", "boolean", "integer", Verdict::Disagree)]
#[case("postgres", "text", "date", Verdict::Disagree)]
// time zones: only two stated zones disagree
#[case(
    "postgres",
    "timestamptz",
    "timestamp without time zone",
    Verdict::Disagree
)]
#[case("postgres", "timestamptz", "timestamp", Verdict::Agree)]
// what the lattice does not model never disagrees
#[case("snowflake", "variant", "varchar", Verdict::Unknown)]
#[case("duckdb", "json", "varchar", Verdict::Unknown)]
#[case("duckdb", "integer[]", "varchar", Verdict::Unknown)]
fn comparisons(
    #[case] dialect: &str,
    #[case] proven: &str,
    #[case] declared: &str,
    #[case] verdict: Verdict,
) {
    assert_eq!(
        compare(&ty(proven, dialect), &ty(declared, dialect)),
        verdict,
        "{proven} vs {declared} on {dialect}"
    );
}

#[test]
fn unknown_and_null_say_nothing() {
    for t in [SqlType::Unknown, SqlType::Null] {
        assert_eq!(compare(&t, &SqlType::Text), Verdict::Unknown);
        assert_eq!(compare(&SqlType::Text, &t), Verdict::Unknown);
    }
}

#[rstest]
#[case("duckdb", SqlType::Int { bits: Some(64) }, Some("bigint"))]
#[case("duckdb", SqlType::Int { bits: Some(32) }, Some("integer"))]
#[case("bigquery", SqlType::Int { bits: Some(64) }, Some("int64"))]
#[case("snowflake", SqlType::Int { bits: Some(64) }, None)]
#[case("duckdb", SqlType::Int { bits: None }, None)]
#[case("duckdb", SqlType::Text, Some("varchar"))]
#[case("postgres", SqlType::Text, Some("text"))]
#[case("bigquery", SqlType::Text, Some("string"))]
#[case("snowflake", SqlType::Decimal { p: Some(10), s: Some(2) }, Some("number(10,2)"))]
#[case("duckdb", SqlType::Decimal { p: Some(10), s: Some(2) }, Some("numeric(10,2)"))]
#[case("duckdb", SqlType::Decimal { p: None, s: None }, None)]
#[case("postgres", SqlType::Float { bits: Some(64) }, Some("double precision"))]
#[case("bigquery", SqlType::Timestamp { tz: Tz::Without }, Some("datetime"))]
#[case("snowflake", SqlType::Timestamp { tz: Tz::With }, Some("timestamp_tz"))]
#[case("duckdb", SqlType::Timestamp { tz: Tz::Unknown }, None)]
#[case("duckdb", SqlType::Bool, Some("boolean"))]
#[case("bigquery", SqlType::Bool, Some("bool"))]
fn renderings(#[case] dialect: &str, #[case] t: SqlType, #[case] want: Option<&str>) {
    assert_eq!(render(&t, &Dialect::new(dialect)).as_deref(), want);
}

/// A rendered type reads back as the type it renders.
#[rstest]
#[case("duckdb")]
#[case("postgres")]
#[case("bigquery")]
#[case("snowflake")]
fn renderings_round_trip(#[case] dialect: &str) {
    let d = Dialect::new(dialect);
    let all = [
        SqlType::Bool,
        SqlType::Int { bits: Some(32) },
        SqlType::Int { bits: Some(64) },
        SqlType::Text,
        SqlType::Date,
        SqlType::Decimal {
            p: Some(12),
            s: Some(3),
        },
        SqlType::Timestamp { tz: Tz::With },
        SqlType::Timestamp { tz: Tz::Without },
    ];
    for t in all {
        let Some(spelled) = render(&t, &d) else {
            continue;
        };
        assert_eq!(
            compare(&t, &parse_sql_type(&spelled, &d)),
            Verdict::Agree,
            "{spelled} on {dialect}"
        );
    }
}

#[test]
fn descriptions_say_what_is_unproven() {
    let d = Dialect::new("duckdb");
    assert_eq!(
        describe(&SqlType::Int { bits: None }, &d),
        "integer (width not proven)"
    );
    assert_eq!(describe(&SqlType::Unknown, &d), "unknown");
    assert_eq!(describe(&SqlType::Int { bits: Some(64) }, &d), "bigint");
}
