//! `dbt.relation` as a query of it, a `.sql` file: Neovim's `:DbtQuery`
//! statement, with the other forms of its yank picker (`relation_forms`) as
//! comments above it — the quoted relation, the `ref()`/`source()` that
//! names it and a query of that.

use serde_json::Value;

use super::text;

/// `"a"."b"."c"`: each part quoted, a `"` doubled; a missing part (a source
/// without a database) left out.
fn quoted(answer: &Value) -> String {
    ["database", "schema", "identifier"]
        .into_iter()
        .filter_map(|key| text(answer, key))
        .map(|part| format!("\"{}\"", part.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(".")
}

/// The Jinja that names the relation.
fn jinja(answer: &Value) -> String {
    let name = text(answer, "name").unwrap_or("?");
    match (text(answer, "kind"), text(answer, "source")) {
        (Some("source"), Some(source)) => format!("{{{{ source('{source}', '{name}') }}}}"),
        _ => format!("{{{{ ref('{name}') }}}}"),
    }
}

/// The query, or `None` without a relation.
pub fn query(answer: &Value, limit: u32) -> Option<String> {
    let relation = text(answer, "relation")?;
    let kind = text(answer, "kind").unwrap_or("relation");
    let name = text(answer, "name").unwrap_or(relation);
    let mut out = format!("-- {kind} {name}: run it with your SQL client\n");
    if text(answer, "origin") == Some("defaults") {
        out.push_str(
            "-- the relation by dbt's defaults; `dbt compile` renders the one it is built in\n",
        );
    }
    let quoted = quoted(answer);
    if !quoted.is_empty() {
        out.push_str(&format!(
            "-- quoted, for names with capitals or spaces: {quoted}\n"
        ));
    }
    let jinja = jinja(answer);
    out.push_str(&format!(
        "-- in a model or an analysis: select * from {jinja}\n\n"
    ));
    out.push_str(&format!("select * from {relation} limit {limit}\n"));
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn a_models_query() {
        let answer = json!({
            "kind": "model", "name": "customers", "source": null, "database": "jaffle",
            "schema": "main", "identifier": "customers", "relation": "jaffle.main.customers",
            "origin": "defaults",
        });
        assert_eq!(
            query(&answer, 100).unwrap(),
            "-- model customers: run it with your SQL client\n\
             -- the relation by dbt's defaults; `dbt compile` renders the one it is built in\n\
             -- quoted, for names with capitals or spaces: \"jaffle\".\"main\".\"customers\"\n\
             -- in a model or an analysis: select * from {{ ref('customers') }}\n\
             \n\
             select * from jaffle.main.customers limit 100\n"
        );
    }

    #[test]
    fn a_sources_query_names_its_source_and_skips_a_missing_database() {
        let answer = json!({
            "kind": "source", "name": "orders", "source": "raw", "database": null,
            "schema": "we\"ird", "identifier": "orders", "relation": "raw_schema.orders",
        });
        let sql = query(&answer, 5).unwrap();
        assert!(sql.contains("\"we\"\"ird\".\"orders\""), "{sql}");
        assert!(sql.contains("{{ source('raw', 'orders') }}"), "{sql}");
        assert!(!sql.contains("defaults"), "{sql}");
        assert!(
            sql.ends_with("select * from raw_schema.orders limit 5\n"),
            "{sql}"
        );
    }

    #[test]
    fn no_relation_no_query() {
        assert_eq!(query(&json!({"kind": "model"}), 100), None);
    }
}
