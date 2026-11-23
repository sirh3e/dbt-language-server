//! `dbt show --output json` stdout → a table. dbt prints its log lines
//! around one JSON object `{"show": [{column: value, …}, …]}` (1.7 adds
//! `"node"`); every row keeps dbt's column order, which a
//! `serde_json::Map` would sort away, hence the ordered row visitor.

use std::fmt;

use cogitator_error::guidance::Guidance;
use serde::de::{Deserializer, MapAccess, Visitor};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShowTable {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
    /// dbt returned more than `limit` rows (the shell asks for one more).
    pub truncated: bool,
}

#[derive(Deserialize)]
struct ShowOutput {
    show: Vec<OrderedRow>,
}

struct OrderedRow(Vec<(String, Value)>);

impl<'de> Deserialize<'de> for OrderedRow {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct RowVisitor;
        impl<'de> Visitor<'de> for RowVisitor {
            type Value = OrderedRow;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a row object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<OrderedRow, A::Error> {
                let mut cells = Vec::new();
                while let Some((key, value)) = map.next_entry::<String, Value>()? {
                    cells.push((key, value));
                }
                Ok(OrderedRow(cells))
            }
        }
        deserializer.deserialize_map(RowVisitor)
    }
}

/// The first `{"show": …}` object of `stdout`, at most `limit` rows.
pub fn parse_show(stdout: &str, limit: usize) -> Result<ShowTable, Guidance> {
    let output = line_starts(stdout)
        .filter(|&at| stdout[at..].trim_start().starts_with('{'))
        .find_map(|at| {
            let mut de = serde_json::Deserializer::from_str(&stdout[at..]);
            ShowOutput::deserialize(&mut de).ok()
        })
        .ok_or_else(|| {
            Guidance::new("dbt show printed no result table.")
                .because("its output held no `{\"show\": […]}` JSON object.")
                .fix("check that the dbt version supports `dbt show --inline --output json` (dbt ≥ 1.5).")
        })?;
    let mut columns: Vec<String> = Vec::new();
    for row in &output.show {
        for (key, _) in &row.0 {
            if !columns.contains(key) {
                columns.push(key.clone());
            }
        }
    }
    let truncated = output.show.len() > limit;
    let rows = output
        .show
        .into_iter()
        .take(limit)
        .map(|OrderedRow(cells)| {
            columns
                .iter()
                .map(|column| {
                    cells
                        .iter()
                        .find(|(key, _)| key == column)
                        .map(|(_, v)| v.clone())
                        .unwrap_or(Value::Null)
                })
                .collect()
        })
        .collect();
    Ok(ShowTable {
        columns,
        rows,
        truncated,
    })
}

fn line_starts(text: &str) -> impl Iterator<Item = usize> + '_ {
    std::iter::once(0).chain(text.match_indices('\n').map(|(i, _)| i + 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rows_keep_dbts_column_order() {
        let cases = [
            (
                "12:00:00  Running with dbt=1.8.0\n12:00:01  Previewing inline node:\n\
                 {\n  \"show\": [\n    {\"zeta\": 1, \"alpha\": \"a\"},\n    {\"zeta\": 2, \"alpha\": null}\n  ]\n}\n",
                10,
                vec!["zeta", "alpha"],
                vec![vec![json!(1), json!("a")], vec![json!(2), Value::Null]],
                false,
            ),
            (
                "{\"node\": \"inline_query\", \"show\": [{\"id\": 1}, {\"id\": 2}, {\"id\": 3}]}",
                2,
                vec!["id"],
                vec![vec![json!(1)], vec![json!(2)]],
                true,
            ),
            ("{\"show\": []}\n", 5, vec![], vec![], false),
            (
                "{\"show\": [{\"a\": 1}, {\"b\": 2}]}",
                5,
                vec!["a", "b"],
                vec![vec![json!(1), Value::Null], vec![Value::Null, json!(2)]],
                false,
            ),
        ];
        for (stdout, limit, columns, rows, truncated) in cases {
            let table = parse_show(stdout, limit).unwrap();
            assert_eq!(table.columns, columns, "{stdout}");
            assert_eq!(table.rows, rows, "{stdout}");
            assert_eq!(table.truncated, truncated, "{stdout}");
        }
    }

    #[test]
    fn log_braces_before_the_table_are_skipped() {
        let stdout = "{not json at all\n{\"other\": 1}\n{\"show\": [{\"x\": true}]}\n";
        let table = parse_show(stdout, 5).unwrap();
        assert_eq!(table.rows, vec![vec![json!(true)]]);
    }

    #[test]
    fn no_table_explains_itself() {
        let err = parse_show("| id |\n| 1 |\n", 5).unwrap_err().to_message();
        assert!(err.contains("no result table"), "{err}");
    }
}
