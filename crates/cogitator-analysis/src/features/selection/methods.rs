//! What each selection method matches on one node, as dbt-core's
//! selector_methods.py does: fqn (with and without the package, and a
//! versioned node's name), tag, path (the properties file too), file,
//! package, resource_type, config.*, access, group, source, exposure,
//! metric, semantic_model, test_type, test_name and unit_test.

use super::glob;
use super::parse::Criterion;
use super::universe::Node;

pub const RESOURCE_TYPES: &[&str] = &[
    "model",
    "seed",
    "snapshot",
    "source",
    "test",
    "unit_test",
    "analysis",
    "exposure",
    "metric",
    "semantic_model",
];

/// Does node `n` meet criterion `c`'s method and value?
pub(super) fn matches(c: &Criterion, n: &Node, root_package: &str) -> bool {
    let v = c.value.as_str();
    let fnm = |text: &str| glob::matches(v, text);
    match c.method.as_str() {
        // dbt's QualifiedNameSelectorMethod: parsed nodes, by their fqn
        // or by it without the package (so `staging` takes a folder)
        "fqn" => {
            let versioned = n.version.is_some();
            matches!(
                n.resource_type,
                "model" | "seed" | "snapshot" | "test" | "analysis"
            ) && (fqn_selected(&n.fqn, v, versioned)
                || fqn_selected(&n.fqn[1.min(n.fqn.len())..], v, versioned))
        }
        "tag" => n.tags.iter().any(|t| fnm(t)),
        "path" => path_selected(&n.path, n.patch_path.as_deref(), v),
        "file" => {
            let base = n.path.rsplit('/').next().unwrap_or_default();
            let stem = base.rsplit_once('.').map_or(base, |(s, _)| s);
            (n.uri.is_some() || !base.is_empty()) && (fnm(base) || fnm(stem))
        }
        "package" => {
            let want = if v == "this" { root_package } else { v };
            glob::matches(want, &n.package)
        }
        "resource_type" => n.resource_type == v,
        "config" => match c.args.join(".").as_str() {
            "materialized" => n.materialized.as_deref().is_some_and(fnm),
            "tags" => n.tags.iter().any(|t| fnm(t)),
            "access" => n.access.as_deref().is_some_and(fnm),
            "group" => n.group.as_deref().is_some_and(fnm),
            _ => false,
        },
        "access" => n.access.as_deref().is_some_and(fnm),
        "group" => n.group.as_deref().is_some_and(fnm),
        "source" => {
            if n.resource_type != "source" {
                return false;
            }
            let source = n.source.as_deref().unwrap_or_default();
            match v.split('.').collect::<Vec<_>>().as_slice() {
                [s] => glob::matches(s, source),
                [s, t] => glob::matches(s, source) && glob::matches(t, &n.name),
                [p, s, t] => {
                    glob::matches(p, &n.package)
                        && glob::matches(s, source)
                        && glob::matches(t, &n.name)
                }
                _ => false,
            }
        }
        "exposure" | "metric" | "semantic_model" => n.resource_type == c.method && fnm(&n.name),
        "unit_test" => {
            n.resource_type == "unit_test" && (fnm(&n.name) || fqn_selected(&n.fqn, v, false))
        }
        "test_type" => match v {
            "data" => matches!(n.test_type, Some("generic" | "singular")),
            other => n.test_type == Some(other),
        },
        "test_name" => n.test_name.as_deref().is_some_and(|t| {
            let bare = t.rsplit('.').next().unwrap_or(t);
            fnm(t) || fnm(bare)
        }),
        _ => false,
    }
}

/// dbt-core's `is_selected_node`: the node's name, or the fqn as a
/// dotted prefix, a part with a wildcard matching the rest of it.
pub fn fqn_selected(fqn: &[String], selector: &str, versioned: bool) -> bool {
    if fqn.is_empty() {
        return false;
    }
    if versioned {
        // the model's name, or `name_v1` / `name.v1` (its last two parts)
        let tail = fqn.len().saturating_sub(2);
        if fqn.len() >= 2 && fqn[tail] == selector {
            return true;
        }
        let parts: Vec<&str> = selector.split('.').collect();
        let last_two = parts[parts.len().saturating_sub(2)..].join("_");
        if fqn[tail..].join("_") == last_two {
            return true;
        }
    } else if fqn.last().is_some_and(|name| name == selector) {
        return true;
    }
    let flat: Vec<&str> = fqn.iter().flat_map(|s| s.split('.')).collect();
    let parts: Vec<&str> = selector.split('.').collect();
    if flat.len() < parts.len() {
        return false;
    }
    for (i, part) in parts.iter().enumerate() {
        if glob::is_glob(part) {
            return glob::matches(&parts[i..].join("."), &flat[i..].join("."));
        }
        if flat[i] != *part {
            return false;
        }
    }
    true
}

/// dbt's path method: the file itself, its properties file, or a
/// directory above it; a glob matches any of those.
pub(super) fn path_selected(path: &str, patch_path: Option<&str>, selector: &str) -> bool {
    let selector = selector.trim_start_matches("./").trim_end_matches('/');
    let hit = |file: &str| {
        if file.is_empty() {
            return false;
        }
        let mut candidates = vec![file.to_string()];
        let mut dir = file;
        while let Some((parent, _)) = dir.rsplit_once('/') {
            candidates.push(parent.to_string());
            dir = parent;
        }
        candidates
            .iter()
            .any(|c| c == selector || glob::matches(selector, c))
    };
    hit(path) || patch_path.is_some_and(hit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn fqn(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    #[rstest]
    #[case(&["vault", "dim", "dim_items"], "dim_items", true)]
    #[case(&["vault", "dim", "dim_items"], "vault", true)]
    #[case(&["vault", "dim", "dim_items"], "vault.dim", true)]
    #[case(&["dim", "dim_items"], "dim", true)]
    #[case(&["vault", "dim", "dim_items"], "dim", false)]
    #[case(&["vault", "dim", "dim_items"], "dim_*", false)]
    #[case(&["vault", "stg_orders"], "stg_*", false)]
    #[case(&["stg_orders"], "stg_*", true)]
    #[case(&["vault", "dim", "dim_items"], "vault.*.dim_items", true)]
    #[case(&["vault", "dim", "dim_items"], "vault.raw_vault", false)]
    #[case(&["vault", "dim", "dim_items"], "vault.dim.dim_items.x", false)]
    fn fqn_selection_follows_dbt(
        #[case] node: &[&str],
        #[case] selector: &str,
        #[case] expected: bool,
    ) {
        assert_eq!(
            fqn_selected(&fqn(node), selector, false),
            expected,
            "{selector}"
        );
    }

    #[rstest]
    #[case("orders", true)]
    #[case("orders_v1", true)]
    #[case("orders.v1", true)]
    #[case("orders_v2", false)]
    #[case("shop", true)]
    fn a_versioned_node_answers_to_its_name_and_version(
        #[case] selector: &str,
        #[case] expected: bool,
    ) {
        assert_eq!(
            fqn_selected(&fqn(&["shop", "orders", "v1"]), selector, true),
            expected,
            "{selector}"
        );
    }

    #[rstest]
    #[case("models/dim/dim_items.sql", None, "models/dim", true)]
    #[case("models/dim/dim_items.sql", None, "models/dim/", true)]
    #[case("models/dim/dim_items.sql", None, "./models", true)]
    #[case("models/dim/dim_items.sql", None, "models/dim/*.sql", true)]
    #[case("models/dim/dim_items.sql", None, "models/raw_vault", false)]
    #[case(
        "models/dim/dim_items.sql",
        Some("models/dim/_dim__models.yml"),
        "models/dim/_dim__models.yml",
        true
    )]
    #[case("models/dimension/x.sql", None, "models/dim", false)]
    fn path_selection_follows_dbt(
        #[case] path: &str,
        #[case] patch: Option<&str>,
        #[case] selector: &str,
        #[case] expected: bool,
    ) {
        assert_eq!(path_selected(path, patch, selector), expected, "{selector}");
    }
}
