//! Exposures, metrics and semantic models as graph citizens (03 §10):
//! parsed LENIENTLY from the marked properties YAML — names, positions,
//! descriptions and every `ref('…')`/`source('…','…')` string in the
//! entry's subtree. dbt keeps reshaping these sections; a raw marked
//! walk survives that where a strict schema type would reject.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use cogitator_lsp::{Position, Range};
use cogitator_schemas::yaml::{as_str, entries, get, items, position_of};
use saphyr::{LoadableYamlNode, MarkedYaml, Scalar, YamlData};

/// What an exposure/metric/semantic model points at.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RefTarget {
    Model(String),
    Source(String, String),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CitizenDetails {
    pub name: String,
    pub uri: PathBuf,
    pub range: Range,
    pub description: Option<String>,
    /// exposures: `type:`; the others leave it None.
    pub citizen_type: Option<String>,
    pub owner: Option<String>,
    pub depends: Vec<RefTarget>,
}

/// One properties file's citizens.
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct CitizenFile {
    pub exposures: Vec<CitizenDetails>,
    pub metrics: Vec<CitizenDetails>,
    pub semantic_models: Vec<CitizenDetails>,
}

/// Parse the three sections out of one properties YAML; anything that
/// does not look like an entry list is skipped silently.
pub fn extract_citizens(text: &str, path: &Path) -> CitizenFile {
    let Ok(docs) = MarkedYaml::load_from_str(text) else {
        return CitizenFile::default();
    };
    let Some(root) = docs.first() else {
        return CitizenFile::default();
    };
    let section = |name: &str| -> Vec<CitizenDetails> {
        let Some(node) = get(root, name) else {
            return Vec::new();
        };
        items(node).filter_map(|item| citizen(item, path)).collect()
    };
    CitizenFile {
        exposures: section("exposures"),
        metrics: section("metrics"),
        semantic_models: section("semantic_models"),
    }
}

fn citizen(item: &MarkedYaml<'_>, path: &Path) -> Option<CitizenDetails> {
    let name_node = get(item, "name")?;
    let name = as_str(name_node)?.to_string();
    let position = position_of(name_node);
    let mut depends = Vec::new();
    collect_refs(item, &mut depends);
    depends.dedup();
    Some(CitizenDetails {
        name,
        uri: path.to_path_buf(),
        range: Range::zero_width(Position {
            line: position.line,
            character: position.character,
        }),
        description: get(item, "description")
            .and_then(as_str)
            .map(str::to_string),
        citizen_type: get(item, "type").and_then(as_str).map(str::to_string),
        owner: get(item, "owner")
            .and_then(|o| get(o, "name").or(Some(o)))
            .and_then(as_str)
            .map(str::to_string),
        depends,
    })
}

/// Every ref()/source() mention in any scalar string of the subtree —
/// `depends_on:`, `model: ref('x')`, wherever dbt puts it this year.
fn collect_refs(node: &MarkedYaml<'_>, out: &mut Vec<RefTarget>) {
    match &node.data {
        YamlData::Value(Scalar::String(text)) => out.extend(refs_in(text)),
        YamlData::Mapping(_) => {
            for (key, value) in entries(node) {
                collect_refs(key, out);
                collect_refs(value, out);
            }
        }
        YamlData::Sequence(_) => {
            for item in items(node) {
                collect_refs(item, out);
            }
        }
        _ => {}
    }
}

/// `ref('x')`, `ref("pkg", "x")` (second arg wins as the model name),
/// `source('a', 'b')` inside one string.
fn refs_in(text: &str) -> Vec<RefTarget> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        let (callee, is_source) = if rest.starts_with("ref(") {
            ("ref(", false)
        } else if rest.starts_with("source(") {
            ("source(", true)
        } else {
            i += 1;
            continue;
        };
        let standalone = i == 0 || (!bytes[i - 1].is_ascii_alphanumeric() && bytes[i - 1] != b'_');
        let after = &rest[callee.len()..];
        let close = after.find(')').unwrap_or(after.len());
        if standalone {
            let args: Vec<String> = after[..close]
                .split(',')
                .filter_map(|a| {
                    let a = a.trim().trim_matches(|c| c == '\'' || c == '"');
                    (!a.is_empty()).then(|| a.to_string())
                })
                .collect();
            match (is_source, args.as_slice()) {
                (false, [model]) => out.push(RefTarget::Model(model.clone())),
                (false, [_pkg, model]) => out.push(RefTarget::Model(model.clone())),
                (true, [source, table]) => {
                    out.push(RefTarget::Source(source.clone(), table.clone()))
                }
                _ => {}
            }
        }
        i += callee.len() + close;
    }
    out
}

/// Merge per-file citizens into the three context maps.
pub fn into_maps(
    files: impl IntoIterator<Item = CitizenFile>,
) -> (
    HashMap<String, CitizenDetails>,
    HashMap<String, CitizenDetails>,
    HashMap<String, CitizenDetails>,
) {
    let (mut exposures, mut metrics, mut semantics) =
        (HashMap::new(), HashMap::new(), HashMap::new());
    for file in files {
        for e in file.exposures {
            exposures.insert(e.name.clone(), e);
        }
        for m in file.metrics {
            metrics.insert(m.name.clone(), m);
        }
        for s in file.semantic_models {
            semantics.insert(s.name.clone(), s);
        }
    }
    (exposures, metrics, semantics)
}

#[cfg(test)]
mod tests {
    use super::*;

    const YAML: &str = "\
version: 2
exposures:
  - name: weekly_kpis
    type: dashboard
    description: the KPI board
    owner:
      name: data team
    depends_on:
      - ref('customers')
      - ref('jaffle_package', 'stg_customer_status')
      - source('stripe', 'payments')
metrics:
  - name: revenue
    model: ref('orders')
semantic_models:
  - name: sm_orders
    model: ref('orders')
";

    #[test]
    fn citizens_parse_with_positions_and_targets() {
        let file = extract_citizens(YAML, Path::new("models/schema.yml"));
        assert_eq!(file.exposures.len(), 1);
        let exposure = &file.exposures[0];
        assert_eq!(exposure.name, "weekly_kpis");
        assert_eq!(exposure.citizen_type.as_deref(), Some("dashboard"));
        assert_eq!(exposure.owner.as_deref(), Some("data team"));
        assert!(exposure.range.start.line > 0);
        assert_eq!(
            exposure.depends,
            vec![
                RefTarget::Model("customers".into()),
                RefTarget::Model("stg_customer_status".into()),
                RefTarget::Source("stripe".into(), "payments".into()),
            ]
        );
        assert_eq!(
            file.metrics[0].depends,
            vec![RefTarget::Model("orders".into())]
        );
        assert_eq!(
            file.semantic_models[0].depends,
            vec![RefTarget::Model("orders".into())]
        );
    }

    #[test]
    fn absent_sections_and_broken_yaml_are_empty() {
        assert!(extract_citizens("models:\n  - name: x\n", Path::new("p"))
            .exposures
            .is_empty());
        assert!(extract_citizens("exposures: [unclosed", Path::new("p"))
            .exposures
            .is_empty());
    }
}
