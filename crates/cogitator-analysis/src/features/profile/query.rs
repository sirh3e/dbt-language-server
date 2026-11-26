//! The profile query: one statement, one result set, so it runs alike
//! through DuckDB and `dbt show --inline`. Rows are tagged by `kind`:
//! one `stats` row (row count, distinct count, null count, min and max
//! as text) and up to ten `top` rows (a value and its count).
//!
//! Sampling, per adapter (a profile is an estimate on large tables):
//! - duckdb: `using sample N rows` (reservoir);
//! - snowflake: `sample (N rows)`;
//! - databricks / spark: `tablesample (N rows)`;
//! - everything else (postgres, redshift, bigquery, …): a `limit N`
//!   subquery — the first N rows the warehouse returns, not a random
//!   sample. On BigQuery a `limit` does not reduce the bytes scanned:
//!   the column is read (and billed) in full.

pub const TOP_VALUES: usize = 10;

/// The text type of a cast, per adapter.
fn text_type(dialect: &str) -> &'static str {
    match dialect {
        "bigquery" | "databricks" | "spark" => "string",
        _ => "varchar",
    }
}

/// `column` quoted for the adapter (embedded quotes doubled).
pub fn quote_identifier(dialect: &str, column: &str) -> String {
    match dialect {
        "bigquery" | "databricks" | "spark" => format!("`{}`", column.replace('`', "``")),
        _ => format!("\"{}\"", column.replace('"', "\"\"")),
    }
}

/// The sampled input: `select <col> as v from …` with the adapter's
/// sampling clause.
fn sampled(dialect: &str, relation: &str, column: &str, rows: u64) -> String {
    let col = quote_identifier(dialect, column);
    match dialect {
        "duckdb" => format!("select {col} as v from {relation} using sample {rows} rows"),
        "snowflake" => format!("select {col} as v from {relation} sample ({rows} rows)"),
        "databricks" | "spark" => {
            format!("select {col} as v from {relation} tablesample ({rows} rows)")
        }
        _ => format!("select {col} as v from {relation} limit {rows}"),
    }
}

pub fn profile_sql(dialect: &str, relation: &str, column: &str, sample_rows: u64) -> String {
    let text = text_type(dialect);
    let sample = sampled(dialect, relation, column, sample_rows.max(1));
    format!(
        "with sampled as ({sample}),\n\
         stats as (select count(*) as n, count(distinct v) as distinct_count, \
         count(*) - count(v) as null_count, cast(min(v) as {text}) as lo, \
         cast(max(v) as {text}) as hi from sampled),\n\
         top_values as (select cast(v as {text}) as value, count(*) as n from sampled \
         where v is not null group by cast(v as {text}) order by n desc limit {TOP_VALUES})\n\
         select 'stats' as kind, lo as value, hi as value2, n, distinct_count, null_count from stats\n\
         union all\n\
         select 'top' as kind, value, null as value2, n, null as distinct_count, null as null_count \
         from top_values"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sampling_follows_the_adapter() {
        let duck = profile_sql("duckdb", "db.main.orders", "amount", 500);
        assert!(
            duck.contains("from db.main.orders using sample 500 rows"),
            "{duck}"
        );
        assert!(duck.contains("cast(min(v) as varchar)"));
        assert!(profile_sql("snowflake", "r", "c", 5).contains("sample (5 rows)"));
        assert!(profile_sql("databricks", "r", "c", 5).contains("tablesample (5 rows)"));
        let bq = profile_sql("bigquery", "`p.d.t`", "c", 5);
        assert!(bq.contains("select `c` as v from `p.d.t` limit 5"), "{bq}");
        assert!(bq.contains("as string"));
        assert!(profile_sql("postgres", "r", "c", 0).contains("limit 1"));
    }

    #[test]
    fn identifiers_are_quoted_and_escaped() {
        assert_eq!(quote_identifier("duckdb", "a\"b"), "\"a\"\"b\"");
        assert_eq!(quote_identifier("bigquery", "a`b"), "`a``b`");
    }
}
