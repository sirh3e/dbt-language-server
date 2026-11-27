//! A model's code as of the last dbt parse or compile: `raw_code` and
//! `compiled_code` from target/manifest.json. Read on demand by
//! dbt.diffAt `rev: "last-run"` — the cached [`super::manifest::ManifestIndex`]
//! deliberately does not hold every model's code.

use std::path::Path;

use cogitator_error::guidance::Guidance;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModelCode {
    pub unique_id: String,
    pub package_name: Option<String>,
    pub original_file_path: Option<String>,
    /// The template as dbt last parsed it.
    pub raw_code: Option<String>,
    /// The SQL dbt last compiled (absent after a bare `dbt parse`).
    pub compiled_code: Option<String>,
}

/// The model named `model` in the manifest text, if any.
pub fn model_code(text: &str, path: &Path, model: &str) -> Result<Option<ModelCode>, Guidance> {
    use cogitator_schemas::artifacts::manifest as gen;
    let raw: gen::ManifestProjection = serde_json::from_str(text).map_err(|err| {
        Guidance::new("could not read manifest.json.")
            .at(path.display().to_string())
            .because(err.to_string())
            .fix("re-run dbt to regenerate target/.")
    })?;
    let mut found = raw.nodes.into_iter().filter(|(unique_id, node)| {
        node.resource_type.as_deref() == Some("model")
            && node
                .name
                .as_deref()
                .unwrap_or_else(|| unique_id.rsplit('.').next().unwrap_or_default())
                == model
    });
    Ok(found.next().map(|(unique_id, node)| ModelCode {
        unique_id,
        package_name: node.package_name,
        original_file_path: node.original_file_path,
        raw_code: node.raw_code,
        compiled_code: node.compiled_code,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_model_and_its_code() {
        let text = r#"{"nodes": {
            "model.p.orders": {"resource_type": "model", "name": "orders", "package_name": "p",
                "original_file_path": "models/orders.sql",
                "raw_code": "select 1", "compiled_code": "select 1 as x"},
            "test.p.t": {"resource_type": "test", "name": "orders"}
        }}"#;
        let code = model_code(text, Path::new("m"), "orders").unwrap().unwrap();
        assert_eq!(code.raw_code.as_deref(), Some("select 1"));
        assert_eq!(code.compiled_code.as_deref(), Some("select 1 as x"));
        assert_eq!(code.package_name.as_deref(), Some("p"));
        assert!(model_code(text, Path::new("m"), "nope").unwrap().is_none());
        assert!(model_code("{", Path::new("m"), "x").is_err());
    }
}
