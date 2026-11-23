//! The preview as a markdown table, for editors that only show
//! documents: a header naming what was previewed and where its SQL came
//! from, the rows, and the query that ran.

use serde_json::Value;

use super::{CtePreview, PreviewSource};

/// `<model>.<cte>.md`, or `<model>.final.md` for the final select.
pub fn preview_file_name(model: &str, cte: Option<&str>) -> String {
    let part: String = cte
        .unwrap_or("final")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{model}.{part}.md")
}

pub fn render_markdown(preview: &CtePreview) -> String {
    let what = match &preview.cte {
        Some(cte) => format!("CTE `{cte}`"),
        None => "final select".to_string(),
    };
    let source = match preview.source {
        PreviewSource::Compiled => "compiled SQL (target/compiled)",
        PreviewSource::Rendered => "SQL rendered by cogitator (no current compiled file)",
    };
    let count = if preview.truncated {
        format!("first {} rows", preview.rows.len())
    } else {
        format!("{} rows", preview.rows.len())
    };
    let mut out = format!(
        "# {} · {what}\n\n{count} · from {source}\n\n",
        preview.model
    );
    if preview.columns.is_empty() {
        out.push_str("_No rows._\n");
    } else {
        let line = |cells: Vec<String>| format!("| {} |\n", cells.join(" | "));
        out.push_str(&line(preview.columns.iter().map(|c| escape(c)).collect()));
        out.push_str(&line(
            preview.columns.iter().map(|_| "---".to_string()).collect(),
        ));
        for row in &preview.rows {
            out.push_str(&line(row.iter().map(cell).collect()));
        }
    }
    out.push_str(&format!("\n```sql\n{}\n```\n", preview.sql));
    out
}

fn cell(value: &Value) -> String {
    match value {
        Value::Null => "NULL".to_string(),
        Value::String(s) => escape(s),
        other => escape(&other.to_string()),
    }
}

fn escape(text: &str) -> String {
    text.replace('|', "\\|").replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn preview(cte: Option<&str>, rows: Vec<Vec<Value>>, truncated: bool) -> CtePreview {
        CtePreview {
            model: "customers".into(),
            cte: cte.map(str::to_string),
            columns: if rows.is_empty() {
                vec![]
            } else {
                vec!["id".into(), "note".into()]
            },
            rows,
            truncated,
            limit: 2,
            sql: "select 1".into(),
            source: PreviewSource::Compiled,
            uri: None,
            table: String::new(),
        }
    }

    #[test]
    fn the_table_escapes_and_says_what_it_shows() {
        let text = render_markdown(&preview(
            Some("orders"),
            vec![vec![json!(1), json!("a|b")], vec![json!(2), Value::Null]],
            true,
        ));
        assert!(
            text.starts_with("# customers · CTE `orders`\n\nfirst 2 rows · from compiled SQL"),
            "{text}"
        );
        assert!(
            text.contains("| id | note |\n| --- | --- |\n| 1 | a\\|b |\n| 2 | NULL |\n"),
            "{text}"
        );
        assert!(text.ends_with("```sql\nselect 1\n```\n"), "{text}");
        let empty = render_markdown(&preview(None, vec![], false));
        assert!(
            empty.contains("final select") && empty.contains("_No rows._"),
            "{empty}"
        );
    }

    #[test]
    fn file_names_are_safe() {
        let cases = [
            ("customers", Some("orders"), "customers.orders.md"),
            ("customers", None, "customers.final.md"),
            ("customers", Some("\"Odd one\""), "customers._Odd_one_.md"),
        ];
        for (model, cte, want) in cases {
            assert_eq!(preview_file_name(model, cte), want);
        }
    }
}
