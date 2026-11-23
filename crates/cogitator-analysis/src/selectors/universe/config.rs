//! A node's resolved config, as dbt layers it: dbt_project.yml's
//! resource section along the node's fqn (deeper wins), then the
//! properties entry's `config:`, then the SQL `config()` call. `tags`
//! add up across layers and `meta` merges, as in dbt.

use serde_json::{Map, Value};

/// Config keys whose value is a mapping: a mapping under one of these
/// names in dbt_project.yml is config even without a `+`, never a
/// directory.
const MAPPING_CONFIGS: &[&str] = &[
    "docs",
    "meta",
    "persist_docs",
    "contract",
    "grants",
    "labels",
    "freshness",
    "quoting",
    "column_types",
    "hours_to_expiration",
    "partition_by",
];

pub type Config = Map<String, Value>;

/// The section of dbt_project.yml for a resource type (`models`, …).
pub fn section<'a>(project: &'a Value, key: &str) -> Option<&'a Value> {
    project.get(key).filter(|v| v.is_object())
}

/// The config a resource section gives the node at `fqn`: every level
/// the fqn walks through contributes its config keys.
pub fn from_project(section: Option<&Value>, fqn: &[String]) -> Config {
    let mut out = Config::new();
    let Some(mut level) = section.and_then(Value::as_object) else {
        return out;
    };
    let mut segments = fqn.iter();
    loop {
        let mut nested = None;
        for (key, value) in level {
            match key.strip_prefix('+') {
                Some(key) => layer_key(&mut out, key, value),
                None if !value.is_object() || MAPPING_CONFIGS.contains(&key.as_str()) => {
                    layer_key(&mut out, key, value)
                }
                None => {}
            }
        }
        if let Some(segment) = segments.next() {
            nested = level
                .get(segment)
                .and_then(Value::as_object)
                .filter(|_| !MAPPING_CONFIGS.contains(&segment.as_str()));
        }
        match nested {
            Some(next) => level = next,
            None => return out,
        }
    }
}

/// Lay `upper` over `base`.
pub fn layer(base: &mut Config, upper: &Config) {
    for (key, value) in upper {
        layer_key(base, key, value);
    }
}

fn layer_key(base: &mut Config, key: &str, value: &Value) {
    match key {
        "tags" => {
            let mut tags = tag_list(base.get("tags"));
            for tag in tag_list(Some(value)) {
                if !tags.contains(&tag) {
                    tags.push(tag);
                }
            }
            base.insert("tags".into(), Value::from(tags));
        }
        "meta" => match (base.get_mut("meta"), value) {
            (Some(Value::Object(old)), Value::Object(new)) => {
                for (k, v) in new {
                    old.insert(k.clone(), v.clone());
                }
            }
            _ => {
                base.insert("meta".into(), value.clone());
            }
        },
        _ => {
            base.insert(key.to_string(), value.clone());
        }
    }
}

/// A `tags:` value: a string or a list of strings.
pub fn tag_list(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

/// dbt's defaults for keys every node has.
pub fn defaults(resource: super::ResourceType) -> Config {
    use super::ResourceType as R;
    let mut out = Config::new();
    let materialized = match resource {
        R::Model => Some("view"),
        R::Seed => Some("seed"),
        R::Snapshot => Some("snapshot"),
        R::Test => Some("test"),
        R::UnitTest => Some("unit"),
        _ => None,
    };
    if let Some(m) = materialized {
        out.insert("materialized".into(), Value::from(m));
    }
    out.insert("enabled".into(), Value::Bool(true));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fqn(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn project_config_follows_the_fqn() {
        let models = json!({
            "jaffle": {
                "materialized": "table",
                "+tags": ["base"],
                "+docs": {"node_color": "gold"},
                "staging": {"materialized": "view", "+tags": "stg", "stg_orders": {"+schema": "raw"}},
                "meta": {"owner": "a"},
            }
        });
        let cases = [
            (
                vec!["jaffle", "customers"],
                json!({"materialized": "table", "tags": ["base"], "docs": {"node_color": "gold"}, "meta": {"owner": "a"}}),
            ),
            (
                vec!["jaffle", "staging", "stg_payments"],
                json!({"materialized": "view", "tags": ["base", "stg"], "docs": {"node_color": "gold"}, "meta": {"owner": "a"}}),
            ),
            (
                vec!["jaffle", "staging", "stg_orders"],
                json!({"materialized": "view", "tags": ["base", "stg"], "docs": {"node_color": "gold"}, "meta": {"owner": "a"}, "schema": "raw"}),
            ),
            (vec!["other", "x"], json!({})),
        ];
        for (path, expected) in cases {
            let got = from_project(Some(&models), &fqn(&path));
            assert_eq!(Value::Object(got), expected, "{path:?}");
        }
    }

    #[test]
    fn layers_add_tags_and_merge_meta() {
        let mut base = json!({"tags": ["a"], "meta": {"x": 1}, "materialized": "view"})
            .as_object()
            .unwrap()
            .clone();
        let upper = json!({"tags": "b", "meta": {"y": 2}, "materialized": "table"})
            .as_object()
            .unwrap()
            .clone();
        layer(&mut base, &upper);
        assert_eq!(
            Value::Object(base),
            json!({"tags": ["a", "b"], "meta": {"x": 1, "y": 2}, "materialized": "table"})
        );
    }
}
