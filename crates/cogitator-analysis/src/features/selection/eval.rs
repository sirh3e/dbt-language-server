//! Evaluate a parsed selection over the universe the way dbt-core's
//! NodeSelector does (graph/selector.py, graph/selector_methods.py):
//! each criterion picks its nodes by method, widens them by its graph
//! operators, then takes in the tests directly on them (indirect
//! selection, "eager"); a group intersects its criteria, the groups are
//! unioned, and the exclusion — evaluated the same way — is subtracted.

use std::collections::{BTreeSet, HashMap};

use super::glob;
use super::parse::{parse, Criterion, Depth, Selection, SelectionError};
use super::universe::{Node, Universe};
use crate::context::SelectorDetails;

pub type Set = BTreeSet<usize>;

pub struct Eval<'a> {
    pub universe: &'a Universe,
    pub selectors: &'a HashMap<String, SelectorDetails>,
    pub root_package: &'a str,
}

impl Eval<'_> {
    /// The nodes of `selection`, without those of `exclude`.
    pub fn select(
        &self,
        selection: &Selection,
        exclude: Option<&Selection>,
    ) -> Result<Set, SelectionError> {
        let mut out = if selection.groups.is_empty() {
            // no selection at all: everything, as `dbt ls` lists it
            (0..self.universe.nodes.len()).collect()
        } else {
            self.union(selection, 0)?
        };
        if let Some(exclude) = exclude {
            for i in self.union(exclude, 0)? {
                out.remove(&i);
            }
        }
        Ok(out)
    }

    /// What an exclusion takes out (nothing for an empty one).
    pub fn excluded(&self, selection: &Selection) -> Result<Set, SelectionError> {
        self.union(selection, 0)
    }

    fn union(&self, selection: &Selection, nesting: usize) -> Result<Set, SelectionError> {
        let mut out = Set::new();
        for group in &selection.groups {
            let mut acc: Option<Set> = None;
            for criterion in group {
                let set = self.criterion(criterion, nesting)?;
                acc = Some(match acc {
                    None => set,
                    Some(prev) => prev.intersection(&set).copied().collect(),
                });
            }
            out.extend(acc.unwrap_or_default());
        }
        Ok(out)
    }

    fn criterion(&self, c: &Criterion, nesting: usize) -> Result<Set, SelectionError> {
        let base = self.method(c, nesting)?;
        let mut out = base.clone();
        if c.childrens_parents {
            let descendants = self.walk(&base, Depth::All, true);
            out.extend(&descendants);
            let mut with_descendants = base.clone();
            with_descendants.extend(descendants);
            out.extend(self.walk(&with_descendants, Depth::All, false));
        }
        if let Some(depth) = c.parents {
            out.extend(self.walk(&base, depth, false));
        }
        if let Some(depth) = c.children {
            out.extend(self.walk(&base, depth, true));
        }
        // eager indirect selection: the tests directly on what is selected;
        // a selector's definition applied it at its own leaves already
        // (its `exclude` would be undone otherwise)
        if c.method == "selector" {
            return Ok(out);
        }
        let tests: Vec<usize> = out
            .iter()
            .flat_map(|&i| self.universe.children(i).iter().copied())
            .filter(|&t| matches!(self.universe.nodes[t].resource_type, "test" | "unit_test"))
            .collect();
        out.extend(tests);
        Ok(out)
    }

    /// Breadth-first ancestors (`down` false) or descendants, `depth` levels.
    fn walk(&self, from: &Set, depth: Depth, down: bool) -> Set {
        let mut seen: Set = from.clone();
        let mut out = Set::new();
        let mut frontier: Vec<usize> = from.iter().copied().collect();
        let mut level = 0u32;
        while !frontier.is_empty() {
            if let Depth::Levels(n) = depth {
                if level >= n {
                    break;
                }
            }
            level += 1;
            let mut next = Vec::new();
            for i in frontier {
                let edges = if down {
                    self.universe.children(i)
                } else {
                    self.universe.parents(i)
                };
                for &j in edges {
                    if seen.insert(j) {
                        out.insert(j);
                        next.push(j);
                    }
                }
            }
            frontier = next;
        }
        out
    }

    fn method(&self, c: &Criterion, nesting: usize) -> Result<Set, SelectionError> {
        let v = c.value.as_str();
        let err = |message: String| SelectionError {
            message,
            start: c.start,
            end: c.end,
        };
        if c.method == "selector" {
            return self.selector(v, nesting).map_err(|e| match e {
                Nested::Unknown => err(format!("selectors.yml has no selector `{v}`")),
                Nested::Cycle => err(format!("selector `{v}` refers back to itself")),
                Nested::Inner(inner) => err(format!("in selector `{v}`: {}", inner.message)),
            });
        }
        if c.method == "version" {
            return Err(err(
                "`version:` is not evaluated in the editor yet".to_string()
            ));
        }
        if c.method == "config" {
            let key = c.args.join(".");
            if !["materialized", "tags", "access", "group"].contains(&key.as_str()) {
                return Err(err(format!(
                    "`config.{key}:` is not known to the editor: config.materialized, config.tags, config.access and config.group are"
                )));
            }
        }
        if c.method == "resource_type" && !RESOURCE_TYPES.contains(&v) {
            return Err(err(format!(
                "`{v}` is no resource type: {}",
                RESOURCE_TYPES.join(", ")
            )));
        }
        if c.method == "test_type" && !["generic", "singular", "unit", "data"].contains(&v) {
            return Err(err(format!(
                "`{v}` is no test type: generic, singular, unit or data"
            )));
        }
        let root = self.root_package;
        Ok(self
            .universe
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| matches(c, n, root))
            .map(|(i, _)| i)
            .collect())
    }

    fn selector(&self, name: &str, nesting: usize) -> Result<Set, Nested> {
        if nesting > 16 {
            return Err(Nested::Cycle);
        }
        let details = self.selectors.get(name).ok_or(Nested::Unknown)?;
        let definition: serde_json::Value =
            serde_json::from_str(&details.definition_text).map_err(|_| Nested::Unknown)?;
        self.definition(&definition, nesting + 1)
    }

    /// A selectors.yml `definition`: a CLI string, a `{method, value}`
    /// mapping (or the `{tag: x}` shorthand), or a union/intersection of
    /// definitions, any of whose items may be an `exclude`.
    fn definition(&self, def: &serde_json::Value, nesting: usize) -> Result<Set, Nested> {
        use serde_json::Value;
        match def {
            Value::String(text) => {
                let sel = parse(text).map_err(Nested::Inner)?;
                self.union(&sel, nesting).map_err(Nested::Inner)
            }
            Value::Object(map) => {
                for (key, combine) in [("union", true), ("intersection", false)] {
                    if let Some(Value::Array(items)) = map.get(key) {
                        let mut acc: Option<Set> = None;
                        let mut excluded = Set::new();
                        for item in items {
                            if let Some(ex) = item.get("exclude") {
                                let list =
                                    ex.as_array().cloned().unwrap_or_else(|| vec![ex.clone()]);
                                for e in &list {
                                    excluded.extend(self.definition(e, nesting)?);
                                }
                                continue;
                            }
                            let set = self.definition(item, nesting)?;
                            acc = Some(match acc {
                                None => set,
                                Some(prev) if combine => prev.union(&set).copied().collect(),
                                Some(prev) => prev.intersection(&set).copied().collect(),
                            });
                        }
                        let mut out = acc.unwrap_or_default();
                        out.retain(|i| !excluded.contains(i));
                        return Ok(out);
                    }
                }
                let (method, value) = match (map.get("method"), map.get("value")) {
                    (Some(m), Some(v)) => (scalar(m), scalar(v)),
                    _ => match map.iter().next() {
                        // `- tag: nightly`
                        Some((m, v)) => (m.clone(), scalar(v)),
                        None => return Ok(Set::new()),
                    },
                };
                let flag = |k: &str| map.get(k).and_then(Value::as_bool).unwrap_or(false);
                let depth_of = |k: &str| {
                    map.get(k)
                        .and_then(Value::as_u64)
                        .map(|n| Depth::Levels(n as u32))
                };
                let mut text = String::new();
                if flag("childrens_parents") {
                    text.push('@');
                }
                let parents = depth_of("parents_depth").or(flag("parents").then_some(Depth::All));
                let children =
                    depth_of("children_depth").or(flag("children").then_some(Depth::All));
                text.push_str(&format!("{method}:{value}"));
                let mut criterion = super::parse::parse(&text)
                    .map_err(Nested::Inner)?
                    .groups
                    .into_iter()
                    .flatten()
                    .next()
                    .ok_or(Nested::Unknown)?;
                criterion.parents = parents;
                criterion.children = children;
                self.criterion(&criterion, nesting).map_err(Nested::Inner)
            }
            _ => Ok(Set::new()),
        }
    }
}

enum Nested {
    Unknown,
    Cycle,
    Inner(SelectionError),
}

fn scalar(v: &serde_json::Value) -> String {
    v.as_str()
        .map(str::to_string)
        .unwrap_or_else(|| v.to_string())
}

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
fn matches(c: &Criterion, n: &Node, root_package: &str) -> bool {
    let v = c.value.as_str();
    let fnm = |text: &str| glob::matches(v, text);
    match c.method.as_str() {
        // dbt's QualifiedNameSelectorMethod: parsed nodes, by their fqn
        // or by it without the package (so `staging` takes a folder)
        "fqn" => {
            matches!(
                n.resource_type,
                "model" | "seed" | "snapshot" | "test" | "analysis"
            ) && (fqn_selected(&n.fqn, v) || fqn_selected(&n.fqn[1.min(n.fqn.len())..], v))
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
        "unit_test" => n.resource_type == "unit_test" && (fnm(&n.name) || fqn_selected(&n.fqn, v)),
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
pub fn fqn_selected(fqn: &[String], selector: &str) -> bool {
    if fqn.is_empty() {
        return false;
    }
    if fqn.last().is_some_and(|name| name == selector) {
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
fn path_selected(path: &str, patch_path: Option<&str>, selector: &str) -> bool {
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
        assert_eq!(fqn_selected(&fqn(node), selector), expected, "{selector}");
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
