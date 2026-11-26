//! dbt.cteSql: the query that selects one CTE (or the final select) of a
//! model, Jinja resolved, to read or run in a SQL client — the preview's
//! query without its limit and without running it. Formatted with the
//! server's own formatter, which declines rather than corrupts: then the
//! query stays as sqlparser printed it.

use serde::Serialize;

use super::PreviewSource;
use crate::features::formatting::{format_document, FormatOptions};

/// The dbt.cteSql answer.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CteSql {
    pub model: String,
    /// `None`: the model's final select.
    pub cte: Option<String>,
    /// `compiled` or `rendered`, as the preview says it.
    pub source: PreviewSource,
    /// The query, formatted.
    pub sql: String,
    /// The query under a header naming what it selects, to show.
    pub text: String,
}

/// The answer for a query cut by `preview_query(.., None)`.
pub fn cte_sql(model: &str, cte: Option<&str>, source: PreviewSource, query: &str) -> CteSql {
    let options = FormatOptions {
        tab_size: 4,
        insert_spaces: true,
        rules: None,
        adapter: String::new(),
    };
    let sql = match format_document(query, &options) {
        Some(edits) if !edits.is_empty() => edits[0].new_text.clone(),
        _ => query.to_string(),
    };
    let sql = format!("{};\n", sql.trim_end().trim_end_matches(';'));
    let what = match cte {
        Some(name) => format!("the CTE {name} of {model}"),
        None => format!("the final select of {model}"),
    };
    let from = match source {
        PreviewSource::Compiled => "dbt's compiled SQL",
        PreviewSource::Rendered => "cogitator's rendering of the model's Jinja",
    };
    let text = format!("-- {what}, from {from}.\n{sql}");
    CteSql {
        model: model.to_string(),
        cte: cte.map(str::to_string),
        source,
        sql,
        text,
    }
}

/// The file `show: true` writes, beside the previews.
pub fn cte_sql_file_name(model: &str, cte: Option<&str>) -> String {
    format!("{model}.{}.sql", cte.unwrap_or("final"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn the_query_is_formatted_under_its_header() {
        let answer = cte_sql(
            "customers",
            Some("orders"),
            PreviewSource::Rendered,
            "WITH orders AS (SELECT order_id, amount FROM shop.main.stg_orders) select * from orders",
        );
        assert!(
            answer
                .text
                .starts_with("-- the CTE orders of customers, from cogitator's rendering"),
            "{}",
            answer.text
        );
        assert!(
            answer.sql.lines().count() > 1,
            "formatted over lines: {}",
            answer.sql
        );
        assert!(answer.sql.ends_with(";\n"), "{}", answer.sql);
        assert!(
            !answer.sql.to_lowercase().contains("limit"),
            "{}",
            answer.sql
        );
        assert_eq!(
            cte_sql_file_name("customers", Some("orders")),
            "customers.orders.sql"
        );
        assert_eq!(cte_sql_file_name("customers", None), "customers.final.sql");
    }
}
