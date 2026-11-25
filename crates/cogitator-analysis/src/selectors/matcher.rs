//! Whether a node matches one criterion's method and value, method by
//! method as dbt-core's `selector_methods.py` searches.

use serde_json::Value;

use super::glob;
use super::method::Method;
use super::universe::{Node, ResourceType, Universe, VersionKind};

/// The nodes a (method, value) selects, before graph operators. `None`
/// for a method this module does not evaluate (selector references,
/// methods needing state, unknown methods): eval reports those.
pub fn search(u: &Universe, method: &Method, value: &str) -> Option<Vec<usize>> {
    let test = |n: &Node| -> bool {
        match method {
            // dbt-core's qualified-name method searches the parsed nodes
            // and exposures, metrics and semantic models, not sources
            // (checked against `dbt ls`, dbt-core 1.12)
            Method::Fqn => {
                (n.resource_type.is_parsed_node()
                    || matches!(
                        n.resource_type,
                        ResourceType::Exposure | ResourceType::Metric | ResourceType::SemanticModel
                    ))
                    && fqn_matches(n, value)
            }
            Method::Tag => n.tags.iter().any(|t| glob::matches(t, value)),
            Method::Group => n.group.as_deref().is_some_and(|g| glob::matches(g, value)),
            Method::Access => {
                n.resource_type == ResourceType::Model && n.access.as_deref() == Some(value)
            }
            Method::Source => source_matches(n, value),
            Method::Path => path_matches(&n.path, value),
            Method::File => file_matches(&n.path, value),
            Method::Package => {
                let value = if value == "this" {
                    u.root_package.as_str()
                } else {
                    value
                };
                glob::matches(&n.package, value)
            }
            Method::Config(key) => config_matches(&n.config, key, value),
            Method::TestName => n
                .test
                .as_ref()
                .and_then(|t| t.test_name.as_deref())
                .is_some_and(|name| glob::matches(name, value)),
            Method::TestType => n.test.as_ref().is_some_and(|t| {
                t.test_type
                    == match value {
                        "schema" => "generic",
                        "data" => "singular",
                        other => other,
                    }
            }),
            Method::ResourceType => n.resource_type.as_str() == value,
            Method::Exposure => named(n, ResourceType::Exposure, value),
            Method::Metric => named(n, ResourceType::Metric, value),
            Method::SemanticModel => named(n, ResourceType::SemanticModel, value),
            Method::UnitTest => named(n, ResourceType::UnitTest, value),
            // the universe holds no saved queries yet
            Method::SavedQuery => false,
            Method::Version => n.resource_type == ResourceType::Model && version_matches(n, value),
            Method::Wildcard => true,
            Method::Selector | Method::NeedsState(_) | Method::Unknown(_) => false,
        }
    };
    match method {
        Method::Selector | Method::NeedsState(_) | Method::Unknown(_) => None,
        _ => Some((0..u.nodes.len()).filter(|&i| test(&u.nodes[i])).collect()),
    }
}

fn named(n: &Node, resource: ResourceType, value: &str) -> bool {
    n.resource_type == resource && glob::matches(&n.name, value)
}

/// dbt's `is_selected_node`: the bare name, `name.vN` for a version, or a
/// dotted prefix of the fqn where a `*` part matches all below it.
/// dbt-core's `node_is_match`: the fqn, or the fqn without its package
/// (a folder or a name in any package).
fn fqn_matches(n: &Node, selector: &str) -> bool {
    let versioned = n.version_number.is_some();
    is_selected(&n.fqn, selector, versioned)
        || (n.fqn.len() > 1 && is_selected(&n.fqn[1..], selector, versioned))
}

/// dbt-core's `is_selected_node`: the name (a versioned model's by its
/// name, or by `name_vN` / `name.vN`), else a prefix of the flattened fqn.
fn is_selected(fqn: &[String], selector: &str, versioned: bool) -> bool {
    let parts: Vec<&str> = selector.split('.').collect();
    if versioned && fqn.len() >= 2 {
        let tail = &fqn[fqn.len() - 2..];
        let wanted = &parts[parts.len().saturating_sub(2)..];
        if glob::matches(&tail[0], selector) || tail.join("_") == wanted.join("_") {
            return true;
        }
    } else if fqn.last().is_some_and(|last| glob::matches(last, selector)) {
        return true;
    }
    let flat: Vec<&str> = fqn.iter().flat_map(|s| s.split('.')).collect();
    if parts.len() > flat.len() {
        return false;
    }
    for (part, wanted) in flat.iter().zip(&parts) {
        if *wanted == "*" {
            return true;
        }
        if !glob::matches(part, wanted) {
            return false;
        }
    }
    true
}

/// `source:src`, `source:src.table`, `source:pkg.src.table`.
fn source_matches(n: &Node, value: &str) -> bool {
    let Some((source, table)) = &n.source else {
        return false;
    };
    let parts: Vec<&str> = value.split('.').collect();
    let (pkg, src, tbl) = match parts.as_slice() {
        [s] => ("*", *s, "*"),
        [s, t] => ("*", *s, *t),
        [p, s, t] => (*p, *s, *t),
        _ => return false,
    };
    glob::matches(&n.package, pkg) && glob::matches(source, src) && glob::matches(table, tbl)
}

/// A file at `path` or under it; the value may be a glob.
fn path_matches(path: &str, value: &str) -> bool {
    let value = value.replace('\\', "/");
    let value = value.trim_start_matches("./").trim_end_matches('/');
    if path.is_empty() || value.is_empty() {
        return false;
    }
    let mut prefix = path;
    loop {
        if glob::matches(prefix, value) {
            return true;
        }
        match prefix.rfind('/') {
            Some(i) => prefix = &prefix[..i],
            None => return false,
        }
    }
}

/// The file's name or its stem.
fn file_matches(path: &str, value: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    if name.is_empty() {
        return false;
    }
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    glob::matches(name, value) || glob::matches(stem, value)
}

/// `config.<key>:<value>`: a dotted key into the config; a list holds
/// the value, a boolean compares case-insensitively.
fn config_matches(config: &serde_json::Map<String, Value>, key: &str, value: &str) -> bool {
    let mut parts = key.split('.');
    let Some(first) = parts.next() else {
        return false;
    };
    let mut current = config.get(first);
    for part in parts {
        current = current.and_then(|v| v.get(part));
    }
    let scalar = |v: &Value| match v {
        Value::String(s) => s == value,
        Value::Bool(b) => b.to_string() == value.to_lowercase(),
        Value::Number(n) => n.to_string() == value,
        _ => false,
    };
    match current {
        Some(Value::Array(items)) => items.iter().any(scalar),
        Some(v) => scalar(v),
        None => false,
    }
}

fn version_matches(n: &Node, value: &str) -> bool {
    matches!(
        (value, n.version),
        ("latest", Some(VersionKind::Latest))
            | ("prerelease", Some(VersionKind::Prerelease))
            | ("old", Some(VersionKind::Old))
            | ("none", None)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(path: &str, fqn: &[&str]) -> Node {
        let mut n = Node::new(ResourceType::Model, fqn.last().unwrap(), fqn[0]);
        n.path = path.to_string();
        n.fqn = fqn.iter().map(|s| s.to_string()).collect();
        n
    }

    #[test]
    fn fqn_cases() {
        let n = node(
            "models/staging/stg_orders.sql",
            &["jaffle", "staging", "stg_orders"],
        );
        let cases = [
            ("stg_orders", true),
            ("stg_*", true),
            ("jaffle", true),
            ("jaffle.staging", true),
            ("jaffle.staging.stg_orders", true),
            ("jaffle.*", true),
            // node_is_match: the fqn without its package, too
            ("staging", true),
            ("staging.stg_orders", true),
            ("marts", false),
            ("jaffle.marts", false),
            ("jaffle.staging.stg_orders.extra", false),
        ];
        for (selector, expected) in cases {
            assert_eq!(fqn_matches(&n, selector), expected, "{selector}");
        }
        let mut v = node("models/co_v1.sql", &["jaffle", "co", "v1"]);
        v.version_number = Some("1".into());
        for (selector, expected) in [
            ("co", true),
            ("co.v1", true),
            ("co.v2", false),
            ("v1", false),
        ] {
            assert_eq!(fqn_matches(&v, selector), expected, "{selector}");
        }
    }

    #[test]
    fn path_and_file_cases() {
        let path = "models/staging/stg_orders.sql";
        let cases = [
            ("models", true),
            ("models/staging", true),
            ("models/staging/", true),
            ("./models/staging", true),
            ("models\\staging", true),
            ("models/staging/stg_orders.sql", true),
            ("models/*/stg_*.sql", true),
            ("models/marts", false),
            ("models/stag", false),
        ];
        for (value, expected) in cases {
            assert_eq!(path_matches(path, value), expected, "{value}");
        }
        for (value, expected) in [
            ("stg_orders.sql", true),
            ("stg_orders", true),
            ("stg_*", true),
            ("orders", false),
        ] {
            assert_eq!(file_matches(path, value), expected, "{value}");
        }
    }

    #[test]
    fn config_cases() {
        let config = serde_json::json!({
            "materialized": "table", "tags": ["a", "b"], "enabled": true,
            "meta": {"owner": "data"}, "threads": 4
        });
        let config = config.as_object().unwrap();
        let cases = [
            ("materialized", "table", true),
            ("materialized", "view", false),
            ("tags", "b", true),
            ("enabled", "True", true),
            ("meta.owner", "data", true),
            ("threads", "4", true),
            ("schema", "x", false),
        ];
        for (key, value, expected) in cases {
            assert_eq!(
                config_matches(config, key, value),
                expected,
                "{key}:{value}"
            );
        }
    }

    #[test]
    fn source_cases() {
        let mut n = Node::new(ResourceType::Source, "orders", "jaffle");
        n.source = Some(("raw".into(), "orders".into()));
        for (value, expected) in [
            ("raw", true),
            ("raw.orders", true),
            ("raw.*", true),
            ("jaffle.raw.orders", true),
            ("raw.payments", false),
            ("other", false),
        ] {
            assert_eq!(source_matches(&n, value), expected, "{value}");
        }
    }
}
