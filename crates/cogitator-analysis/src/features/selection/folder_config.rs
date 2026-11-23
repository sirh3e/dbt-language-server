//! Folder-level configs of `dbt_project.yml` (`models:`, `seeds:`,
//! `snapshots:`): the tree under the package's name, one level per
//! folder, each level's `+materialized` overriding the one above and its
//! `+tags` adding to them, as dbt resolves them. A key without the `+` is
//! a config only when it is a config dbt knows (otherwise a folder).

use serde_json::Value;

/// The configs a node in `folders` (path components below the resource
/// path, without the file) of `package` takes from `tree`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FolderConfig {
    pub materialized: Option<String>,
    pub tags: Vec<String>,
}

const CONFIG_KEYS: &[&str] = &["materialized", "tags"];

pub fn resolve(tree: Option<&Value>, package: &str, folders: &[&str]) -> FolderConfig {
    let mut out = FolderConfig::default();
    let Some(tree) = tree else {
        return out;
    };
    // configs directly under `models:` apply to every package
    apply(tree, &mut out);
    let Some(mut level) = child(tree, package) else {
        return out;
    };
    apply(level, &mut out);
    for folder in folders {
        match child(level, folder) {
            Some(next) => {
                apply(next, &mut out);
                level = next;
            }
            None => break,
        }
    }
    out
}

/// A sub-tree: a key that is no config.
fn child<'a>(level: &'a Value, key: &str) -> Option<&'a Value> {
    if key.starts_with('+') || CONFIG_KEYS.contains(&key) {
        return None;
    }
    level.get(key).filter(|v| v.is_object())
}

fn apply(level: &Value, out: &mut FolderConfig) {
    let Some(map) = level.as_object() else {
        return;
    };
    for key in ["+materialized", "materialized"] {
        if let Some(m) = map.get(key).and_then(Value::as_str) {
            out.materialized = Some(m.to_string());
        }
    }
    for key in ["+tags", "tags"] {
        match map.get(key) {
            Some(Value::String(tag)) => push(out, tag),
            Some(Value::Array(tags)) => {
                for tag in tags.iter().filter_map(Value::as_str) {
                    push(out, tag);
                }
            }
            _ => {}
        }
    }
}

fn push(out: &mut FolderConfig, tag: &str) {
    if !out.tags.iter().any(|t| t == tag) {
        out.tags.push(tag.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn deeper_folders_override_materialized_and_add_tags() {
        let tree = json!({
            "+tags": ["everywhere"],
            "vault": {
                "+materialized": "view",
                "raw_vault": {"+materialized": "incremental", "+tags": "raw"},
                "dim": {"materialized": "table", "tags": ["mart", "raw"]}
            }
        });
        assert_eq!(
            resolve(Some(&tree), "vault", &["raw_vault", "hubs"]),
            FolderConfig {
                materialized: Some("incremental".into()),
                tags: vec!["everywhere".into(), "raw".into()]
            }
        );
        assert_eq!(
            resolve(Some(&tree), "vault", &["dim"]),
            FolderConfig {
                materialized: Some("table".into()),
                tags: vec!["everywhere".into(), "mart".into(), "raw".into()]
            }
        );
        assert_eq!(
            resolve(Some(&tree), "vault", &[]).materialized.as_deref(),
            Some("view")
        );
        assert_eq!(
            resolve(Some(&tree), "other", &["dim"]).tags,
            vec!["everywhere".to_string()]
        );
        assert_eq!(resolve(None, "vault", &["dim"]), FolderConfig::default());
    }
}
