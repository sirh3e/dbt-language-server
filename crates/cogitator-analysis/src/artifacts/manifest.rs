//! manifest.json, reduced to the two lookups the features need: model
//! name → node, and test unique_id → the model it attaches to. The
//! rest of the (huge) manifest is never materialized.

use std::collections::HashMap;
use std::path::Path;

use cogitator_error::guidance::Guidance;
use serde::Deserialize;

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

#[derive(Debug, Deserialize)]
struct RawManifest {
    #[serde(default)]
    nodes: HashMap<String, RawNode>,
}

#[derive(Debug, Deserialize)]
struct RawNode {
    #[serde(default)]
    resource_type: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    original_file_path: Option<String>,
    #[serde(default)]
    attached_node: Option<String>,
}

pub fn parse(text: &str, path: &Path) -> Result<ManifestIndex, Guidance> {
    let raw: RawManifest = serde_json::from_str(text).map_err(|err| {
        Guidance::new("could not read manifest.json.")
            .at(path.display().to_string())
            .because(err.to_string())
            .fix("re-run dbt to regenerate target/; manifest cross-checks stay off until then.")
    })?;
    let mut index = ManifestIndex::default();
    for (unique_id, node) in raw.nodes {
        match node.resource_type.as_str() {
            "model" => {
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
            "test" => {
                index.tests.insert(
                    unique_id,
                    TestNode {
                        attached_node: node.attached_node,
                        name: node.name,
                    },
                );
            }
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
}
