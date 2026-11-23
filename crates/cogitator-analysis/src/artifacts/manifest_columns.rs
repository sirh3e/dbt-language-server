//! A model's documented `columns` from manifest.json — the schema as of
//! the last `dbt parse`/`build`, which the breaking-change preview
//! compares with the current SQL. Arrives as a `serde_json::Value`
//! (`{name: {name, data_type, …}}`); anything unexpected drops the
//! one column, never the manifest.

use serde_json::Value;

/// One documented column as the manifest recorded it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestColumn {
    pub name: String,
    /// `data_type:` as documented; `None` when absent or empty.
    pub data_type: Option<String>,
}

/// The documented columns, sorted by name: the manifest's key order is
/// not stable across dbt versions.
pub(super) fn parse_columns(columns: Value) -> Vec<ManifestColumn> {
    let Value::Object(map) = columns else {
        return Vec::new();
    };
    let mut out: Vec<ManifestColumn> = map
        .into_iter()
        .filter_map(|(key, column)| {
            let name = column
                .get("name")
                .and_then(Value::as_str)
                .filter(|n| !n.trim().is_empty())
                .map(str::to_string)
                .or_else(|| (!key.trim().is_empty()).then_some(key))?;
            let data_type = column
                .get("data_type")
                .and_then(Value::as_str)
                .filter(|t| !t.trim().is_empty())
                .map(str::to_string);
            Some(ManifestColumn { name, data_type })
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn columns_read_leniently() {
        let cases = [
            (json!(null), vec![]),
            (json!([]), vec![]),
            (
                json!({"b": {"name": "b", "data_type": "INT"}, "a": {}, "": {"name": ""}}),
                vec![("a", None), ("b", Some("INT"))],
            ),
            (
                json!({"k": {"name": "real", "data_type": " "}}),
                vec![("real", None)],
            ),
        ];
        for (input, expected) in cases {
            let got: Vec<(String, Option<String>)> = parse_columns(input.clone())
                .into_iter()
                .map(|c| (c.name, c.data_type))
                .collect();
            let expected: Vec<(String, Option<String>)> = expected
                .into_iter()
                .map(|(n, t)| (n.to_string(), t.map(str::to_string)))
                .collect();
            assert_eq!(got, expected, "{input}");
        }
    }
}
