//! The SQL a preview runs: the model's compiled text parsed with
//! sqlparser, its WITH clause kept up to the chosen CTE and the final
//! select replaced by `select * from <cte> limit N` — rebuilt from the
//! AST, never by slicing the text. Only a single SELECT-shaped query is
//! accepted (no DML, DDL, `SELECT … INTO` or row locks), so a preview
//! can only read.

use cogitator_dialects::Dialect;
use cogitator_error::guidance::{closest_matches, Guidance};
use cogitator_parser::backend::sql_dialect;
use sqlparser::ast::{Query, SetExpr, Statement};
use sqlparser::parser::Parser;

/// The query previewing `cte` (or, for `None`, the model's final select)
/// of `compiled`, limited to `limit` rows.
pub fn preview_query(
    compiled: &str,
    dialect: &Dialect,
    cte: Option<&str>,
    limit: Option<usize>,
) -> Result<String, Guidance> {
    let query = single_query(compiled, dialect)?;
    // no limit: the SQL as a reader runs it (dbt.cteSql)
    let limit = limit.map(|n| format!(" limit {n}")).unwrap_or_default();
    let sql = match cte {
        None => format!("select * from ({query}) as cogitator_preview{limit}"),
        Some(name) => {
            let wanted = unquote(name);
            let with = query.with.as_ref().ok_or_else(|| no_such_cte(name, &[]))?;
            let index = with
                .cte_tables
                .iter()
                .position(|c| c.alias.name.value.eq_ignore_ascii_case(wanted))
                .ok_or_else(|| {
                    let names: Vec<String> = with
                        .cte_tables
                        .iter()
                        .map(|c| c.alias.name.value.clone())
                        .collect();
                    no_such_cte(name, &names)
                })?;
            let mut kept = with.clone();
            kept.cte_tables.truncate(index + 1);
            let alias = kept.cte_tables[index].alias.name.clone();
            format!("{kept} select * from {alias}{limit}")
        }
    };
    // what we print must read back as one query, or we would run
    // something other than what was checked
    single_query(&sql, dialect)?;
    Ok(sql)
}

fn unquote(name: &str) -> &str {
    name.trim_matches(|c| c == '"' || c == '`' || c == '[' || c == ']')
}

fn single_query(text: &str, dialect: &Dialect) -> Result<Box<Query>, Guidance> {
    let parser_dialect = sql_dialect(dialect);
    let statements = Parser::parse_sql(parser_dialect.as_ref(), text).map_err(|err| {
        Guidance::new("The model's SQL could not be parsed for a preview.")
            .because(format!("{err}."))
            .fix("compile the model (dbt compile) and fix what dbt reports, then preview again.")
    })?;
    let mut statements = statements.into_iter();
    let (Some(first), None) = (statements.next(), statements.next()) else {
        return Err(not_select("it is not exactly one statement"));
    };
    let Statement::Query(query) = first else {
        return Err(not_select("it is not a query"));
    };
    if !reads_only(&query) {
        return Err(not_select(
            "it writes (SELECT … INTO) or locks rows (FOR UPDATE)",
        ));
    }
    Ok(query)
}

/// No `SELECT … INTO` and no row locks anywhere a query nests.
fn reads_only(query: &Query) -> bool {
    query.locks.is_empty()
        && query
            .with
            .as_ref()
            .is_none_or(|w| w.cte_tables.iter().all(|c| reads_only(&c.query)))
        && body_reads_only(&query.body)
}

fn body_reads_only(body: &SetExpr) -> bool {
    match body {
        SetExpr::Select(select) => select.into.is_none(),
        SetExpr::Query(query) => reads_only(query),
        SetExpr::SetOperation { left, right, .. } => {
            body_reads_only(left) && body_reads_only(right)
        }
        SetExpr::Values(_) => true,
        _ => false,
    }
}

fn not_select(why: &str) -> Guidance {
    Guidance::new("Only a SELECT can be previewed.")
        .because(format!("the model's SQL is refused: {why}."))
        .fix("preview models whose SQL is one SELECT (with CTEs); hooks and DDL never run.")
}

fn no_such_cte(name: &str, names: &[String]) -> Guidance {
    Guidance::new(format!("The model's SQL has no CTE '{name}'."))
        .because(if names.is_empty() {
            "its query has no WITH clause.".to_string()
        } else {
            format!("its CTEs are {}.", names.join(", "))
        })
        .suggest(closest_matches(name, names.iter().map(String::as_str)))
        .fix("pick a CTE of the model's top-level WITH clause, or recompile the model.")
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODEL: &str = "with orders as (select 1 as id), payments as (select id from orders), \
                         final as (select * from payments) select * from final order by id";

    fn duck() -> Dialect {
        Dialect::new("duckdb")
    }

    #[test]
    fn a_cte_keeps_the_with_clause_up_to_itself() {
        let cases: [(Option<&str>, &str); 4] = [
            (
                Some("orders"),
                "WITH orders AS (SELECT 1 AS id) select * from orders limit 5",
            ),
            (
                Some("PAYMENTS"),
                "WITH orders AS (SELECT 1 AS id), payments AS (SELECT id FROM orders) \
                 select * from payments limit 5",
            ),
            (
                Some("final"),
                "WITH orders AS (SELECT 1 AS id), payments AS (SELECT id FROM orders), \
                 final AS (SELECT * FROM payments) select * from final limit 5",
            ),
            (
                None,
                "select * from (WITH orders AS (SELECT 1 AS id), payments AS (SELECT id FROM orders), \
                 final AS (SELECT * FROM payments) SELECT * FROM final ORDER BY id) \
                 as cogitator_preview limit 5",
            ),
        ];
        for (cte, want) in cases {
            assert_eq!(
                preview_query(MODEL, &duck(), cte, Some(5)).unwrap(),
                want,
                "{cte:?}"
            );
        }
    }

    #[test]
    fn what_does_not_only_read_is_refused() {
        let cases = [
            ("insert into t select 1", "not a query"),
            ("drop table t", "not a query"),
            ("select 1; select 2", "exactly one statement"),
            ("select 1 into t", "INTO"),
            ("with a as (select 1 into t) select * from a", "INTO"),
            ("select * from t for update", "locks"),
        ];
        for (sql, why) in cases {
            let err = preview_query(sql, &Dialect::new("postgres"), None, Some(5))
                .unwrap_err()
                .to_message();
            assert!(
                err.contains("Only a SELECT") && err.contains(why),
                "{sql}: {err}"
            );
        }
    }

    #[test]
    fn an_unknown_cte_names_the_ones_there_are() {
        let err = preview_query(MODEL, &duck(), Some("payment"), Some(5))
            .unwrap_err()
            .to_message();
        assert!(err.contains("no CTE 'payment'"), "{err}");
        assert!(err.contains("orders, payments, final"), "{err}");
        let none = preview_query("select 1", &duck(), Some("x"), Some(5))
            .unwrap_err()
            .to_message();
        assert!(none.contains("no WITH clause"), "{none}");
    }

    #[test]
    fn unparsable_sql_says_so() {
        let err = preview_query("select from where", &duck(), None, Some(5))
            .unwrap_err()
            .to_message();
        assert!(err.contains("could not be parsed"), "{err}");
    }

    #[test]
    fn a_quoted_cte_name_matches() {
        let sql = preview_query(
            "with \"Odd\" as (select 1) select * from \"Odd\"",
            &duck(),
            Some("\"Odd\""),
            Some(3),
        )
        .unwrap();
        assert_eq!(
            sql,
            "WITH \"Odd\" AS (SELECT 1) select * from \"Odd\" limit 3"
        );
    }

    #[test]
    fn without_a_limit_the_cte_is_selected_whole() {
        let sql = preview_query(
            "with a as (select 1 as x), b as (select x from a) select * from b",
            &duck(),
            Some("a"),
            None,
        )
        .unwrap();
        assert_eq!(sql, "WITH a AS (SELECT 1 AS x) select * from a");
    }
}
