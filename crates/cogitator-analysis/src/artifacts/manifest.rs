//! manifest.json, reduced to the two lookups the features need: model
//! name → node, and test unique_id → the model it attaches to —
//! deserialized through the generated PROJECTION of the official v12
//! schema (08 §4); the rest of the 946 KB manifest never materializes.

use std::collections::HashMap;
use std::path::Path;

use cogitator_error::guidance::Guidance;
#[derive(Debug, Default)]
pub struct ManifestIndex {
    pub models: HashMap<String, ManifestNode>,
    /// test unique_id → test node.
    pub tests: HashMap<String, TestNode>,
}

#[derive(Debug)]
pub struct ManifestNode {
    pub unique_id: String,
    pub original_file_path: Option<String>,
}

#[derive(Debug)]
pub struct TestNode {
    /// The model unique_id the test attaches to.
    pub attached_node: Option<String>,
    pub name: Option<String>,
}

pub fn parse(text: &str, path: &Path) -> Result<ManifestIndex, Guidance> {
    use cogitator_schemas::artifacts::manifest as gen;
    let raw: gen::ManifestProjection = serde_json::from_str(text).map_err(|err| {
        Guidance::new("could not read manifest.json.")
            .at(path.display().to_string())
            .because(err.to_string())
            .fix("re-run dbt to regenerate target/; manifest cross-checks stay off until then.")
    })?;
    super::trace_schema_version(
        path,
        raw.metadata
            .as_ref()
            .and_then(|m| m.dbt_schema_version.as_deref()),
        cogitator_schemas::artifacts::MANIFEST_SCHEMA_V12,
    );
    let mut index = ManifestIndex::default();
    for (unique_id, node) in raw.nodes {
        match node.resource_type.as_deref() {
            Some("model") => {
                let name = node.name.unwrap_or_else(|| {
                    unique_id.rsplit('.').next().unwrap_or_default().to_string()
                });
                index.models.insert(
                    name,
                    ManifestNode {
                        unique_id,
                        original_file_path: node.original_file_path,
                    },
                );
            }
            Some("test") => {
                index.tests.insert(
                    unique_id,
                    TestNode {
                        attached_node: node.attached_node,
                        name: node.name,
                    },
                );
            }
            // other resource kinds — and nodes without one — are not ours
            _ => {}
        }
    }
    Ok(index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_models_and_tests_only() {
        let index = parse(
            r#"{"nodes": {
                "model.jaffle_shop.customers": {"resource_type": "model", "name": "customers",
                    "original_file_path": "models/customers.sql", "config": {"alias": null}},
                "test.jaffle_shop.unique_x.abc": {"resource_type": "test",
                    "attached_node": "model.jaffle_shop.customers", "name": "unique_x"},
                "seed.jaffle_shop.raw": {"resource_type": "seed"}
            }, "sources": {}, "metadata": {}}"#,
            Path::new("manifest.json"),
        )
        .unwrap();
        assert_eq!(index.models.len(), 1);
        assert_eq!(index.tests.len(), 1);
        assert_eq!(
            index.tests["test.jaffle_shop.unique_x.abc"]
                .attached_node
                .as_deref(),
            Some("model.jaffle_shop.customers")
        );
    }

    #[test]
    fn nodes_without_resource_type_are_skipped() {
        let index = parse(
            r#"{"nodes": {
                "mystery.p.x": {"name": "x"},
                "model.p.m": {"resource_type": "model", "name": "m"}
            }}"#,
            Path::new("m"),
        )
        .unwrap();
        assert_eq!(index.models.len(), 1);
        assert!(index.tests.is_empty());
    }
}
