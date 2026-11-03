//! target/catalog.json (dbt docs generate), through the generated
//! catalog-v1 types: warehouse column metadata for every node and
//! source — the adapter-agnostic sibling of the duckdb introspection
//! pass (03 §10).

use std::collections::HashMap;
use std::path::Path;

use cogitator_error::guidance::Guidance;

#[derive(Debug, Default)]
pub struct CatalogIndex {
    /// (source name, table name) from "source.<proj>.<source>.<table>".
    pub sources: HashMap<(String, String), CatalogTable>,
    /// model name (last unique_id segment) → table.
    pub models: HashMap<String, CatalogTable>,
}

#[derive(Debug, Default)]
pub struct CatalogTable {
    pub schema: Option<String>,
    pub name: Option<String>,
    /// Sorted by warehouse ordinal, then name — stable output.
    pub columns: Vec<CatalogColumn>,
}

#[derive(Debug)]
pub struct CatalogColumn {
    pub name: String,
    pub data_type: String,
    pub comment: Option<String>,
    pub index: Option<i64>,
}

pub fn parse(text: &str, path: &Path) -> Result<CatalogIndex, Guidance> {
    use cogitator_schemas::artifacts::catalog as gen;
    let raw: gen::CatalogArtifact = serde_json::from_str(text).map_err(|err| {
        Guidance::new("could not read catalog.json.")
            .at(path.display().to_string())
            .because(err.to_string())
            .fix("re-run `dbt docs generate`; catalog-backed columns stay off until then.")
    })?;
    super::trace_schema_version(
        path,
        raw.metadata
            .as_ref()
            .and_then(|m| m.dbt_schema_version.as_deref()),
        cogitator_schemas::artifacts::CATALOG_SCHEMA_V1,
    );
    let mut index = CatalogIndex::default();
    for (unique_id, table) in raw.sources {
        let parts: Vec<&str> = unique_id.split('.').collect();
        let ["source", _, source, table_name] = parts.as_slice() else {
            continue;
        };
        index
            .sources
            .insert((source.to_string(), table_name.to_string()), convert(table));
    }
    for (unique_id, table) in raw.nodes {
        let Some(name) = unique_id.rsplit('.').next() else {
            continue;
        };
        index.models.insert(name.to_string(), convert(table));
    }
    Ok(index)
}

fn convert(table: cogitator_schemas::artifacts::catalog::CatalogTable) -> CatalogTable {
    let metadata = table.metadata;
    let mut columns: Vec<CatalogColumn> = table
        .columns
        .into_iter()
        .filter_map(|(key, column)| {
            let name = column.name.or(Some(key)).filter(|n| !n.is_empty());
            let Some(name) = name else {
                tracing::debug!("catalog column without a name skipped");
                return None;
            };
            Some(CatalogColumn {
                name,
                data_type: column.type_.unwrap_or_default(),
                comment: column.comment,
                index: column.index,
            })
        })
        .collect();
    columns.sort_by(|a, b| (a.index, &a.name).cmp(&(b.index, &b.name)));
    CatalogTable {
        schema: metadata.as_ref().and_then(|m| m.schema.clone()),
        name: metadata.and_then(|m| m.name),
        columns,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_sources_and_models_with_sorted_columns() {
        let index = parse(
            r#"{"errors": null,
                "nodes": {"model.jaffle.customers": {
                    "metadata": {"type": "VIEW", "schema": "main", "name": "customers"},
                    "columns": {"id": {"type": "BIGINT", "index": 1, "name": "id"}},
                    "stats": {}}},
                "sources": {"source.jaffle.stripe.payments": {
                    "metadata": {"type": "BASE TABLE", "schema": "main", "name": "payments"},
                    "columns": {
                        "amount": {"type": "INTEGER", "index": 2, "name": "amount"},
                        "": {"type": "GHOST", "index": 9, "name": ""},
                        "id": {"type": "BIGINT", "index": 1, "name": "id", "comment": "pk"}},
                    "stats": {}}}}"#,
            Path::new("catalog.json"),
        )
        .unwrap();
        let payments = &index.sources[&("stripe".to_string(), "payments".to_string())];
        let names: Vec<&str> = payments.columns.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["id", "amount"],
            "ordinal-sorted, nameless dropped"
        );
        assert_eq!(payments.columns[0].comment.as_deref(), Some("pk"));
        assert_eq!(index.models["customers"].columns.len(), 1);
    }

    #[test]
    fn broken_json_is_guidance() {
        let err = parse("{nope", Path::new("target/catalog.json")).unwrap_err();
        assert!(
            err.to_message().contains("dbt docs generate"),
            "{}",
            err.to_message()
        );
    }
}
