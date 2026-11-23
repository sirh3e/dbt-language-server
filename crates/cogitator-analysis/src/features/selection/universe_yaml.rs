//! What properties files say of the nodes beyond the context's maps:
//! model versions (`versions:`, `defined_in:`, `latest_version:`), which
//! dbt makes a node `name.vN` each, and snapshots defined in YAML (dbt
//! 1.9's `snapshots:` with a `relation:`), which no `{% snapshot %}`
//! block shows.

use cogitator_lsp::Range;
use cogitator_schemas::yaml::{as_str, get, items, position_of, to_json};
use saphyr::{LoadableYamlNode, MarkedYaml};

use crate::features::quality::test_nodes::{referenced, TestParent};

/// One version of a model: the file stem it is defined in, and `vN`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelVersion {
    pub model: String,
    pub version: String,
    /// The file stem: `defined_in`, else `<model>_v<N>`.
    pub file: String,
    pub latest: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct YamlSnapshot {
    pub name: String,
    pub parents: Vec<TestParent>,
    pub tags: Vec<String>,
    pub range: Range,
}

fn root_of(text: &str) -> Option<MarkedYaml<'_>> {
    MarkedYaml::load_from_str(text).ok()?.into_iter().next()
}

fn scalar(node: &MarkedYaml<'_>) -> Option<String> {
    match to_json(node) {
        serde_json::Value::String(s) => Some(s),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

pub fn model_versions(text: &str) -> Vec<ModelVersion> {
    let Some(root) = root_of(text) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in get(&root, "models").into_iter().flat_map(items) {
        let Some(model) = get(entry, "name").and_then(as_str) else {
            continue;
        };
        let versions: Vec<String> = get(entry, "versions")
            .into_iter()
            .flat_map(items)
            .filter_map(|v| get(v, "v").and_then(scalar))
            .collect();
        // dbt's latest: `latest_version:`, else the highest
        let latest = get(entry, "latest_version").and_then(scalar).or_else(|| {
            versions
                .iter()
                .max_by(|a, b| {
                    a.parse::<f64>()
                        .unwrap_or(0.0)
                        .total_cmp(&b.parse::<f64>().unwrap_or(0.0))
                })
                .cloned()
        });
        for item in get(entry, "versions").into_iter().flat_map(items) {
            let Some(version) = get(item, "v").and_then(scalar) else {
                continue;
            };
            let file = get(item, "defined_in")
                .and_then(as_str)
                .map(str::to_string)
                .unwrap_or_else(|| format!("{model}_v{version}"));
            out.push(ModelVersion {
                model: model.to_string(),
                latest: latest.as_deref() == Some(version.as_str()),
                version,
                file,
            });
        }
    }
    out
}

pub fn yaml_snapshots(text: &str) -> Vec<YamlSnapshot> {
    let Some(root) = root_of(text) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in get(&root, "snapshots").into_iter().flat_map(items) {
        let (Some(name_node), Some(relation)) =
            (get(entry, "name"), get(entry, "relation").and_then(as_str))
        else {
            continue;
        };
        let Some(name) = as_str(name_node) else {
            continue;
        };
        let tags = match get(entry, "config")
            .and_then(|c| get(c, "tags"))
            .map(to_json)
        {
            Some(serde_json::Value::String(t)) => vec![t],
            Some(serde_json::Value::Array(ts)) => ts
                .iter()
                .filter_map(|t| t.as_str().map(str::to_string))
                .collect(),
            _ => Vec::new(),
        };
        out.push(YamlSnapshot {
            name: name.to_string(),
            parents: referenced(relation),
            tags,
            range: Range::zero_width(position_of(name_node)),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn versions_name_their_files_and_the_latest() {
        let text = "models:\n  - name: orders\n    latest_version: 1\n    versions:\n      - v: 1\n        defined_in: orders\n      - v: 2\n";
        assert_eq!(
            model_versions(text),
            vec![
                ModelVersion {
                    model: "orders".into(),
                    version: "1".into(),
                    file: "orders".into(),
                    latest: true
                },
                ModelVersion {
                    model: "orders".into(),
                    version: "2".into(),
                    file: "orders_v2".into(),
                    latest: false
                },
            ]
        );
    }

    #[test]
    fn a_yaml_snapshot_names_its_relation() {
        let text = "snapshots:\n  - name: s\n    relation: source('raw', 'items')\n    config:\n      tags: [history]\n";
        let snaps = yaml_snapshots(text);
        assert_eq!(snaps.len(), 1);
        assert_eq!(
            snaps[0].parents,
            vec![TestParent::Source("raw".into(), "items".into())]
        );
        assert_eq!(snaps[0].tags, vec!["history".to_string()]);
    }
}
