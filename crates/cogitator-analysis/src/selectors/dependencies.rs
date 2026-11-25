//! dbt.modelDependencies: one model's neighbours in the project, for the
//! dependencies dialog — what it uses directly and what uses it directly,
//! then everything upstream and downstream of it, nearest first — each
//! node with the config that says how it is built (materialized, the
//! unique and partition keys, the incremental strategy), resolved as dbt
//! layers it (`universe::config`). Pure: from the universe.

use std::collections::VecDeque;

use serde::Serialize;
use serde_json::Value;

use super::universe::{ResourceType, Universe};
use crate::features::file_uri;

/// How a node is built, from its resolved config.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildConfig {
    pub materialized: Option<String>,
    pub incremental_strategy: Option<String>,
    /// `unique_key`: one column or several.
    pub unique_key: Vec<String>,
    /// `partition_by`: the columns, or BigQuery's `{field}`.
    pub partition_by: Vec<String>,
    pub cluster_by: Vec<String>,
    pub on_schema_change: Option<String>,
}

/// One node of the answer.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyNode {
    pub name: String,
    pub kind: &'static str,
    pub package: String,
    pub uri: Option<String>,
    /// 0-based line of the definition (a source's YAML entry, a macro…).
    pub line: u32,
    /// Hops from the model (1 = direct).
    pub depth: u32,
    pub tags: Vec<String>,
    pub config: BuildConfig,
}

/// The dbt.modelDependencies result.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dependencies {
    pub model: DependencyNode,
    /// What the model reads directly (its refs and sources).
    pub parents: Vec<DependencyNode>,
    /// What reads the model directly.
    pub children: Vec<DependencyNode>,
    /// Everything upstream, nearest first (the parents included).
    pub upstream: Vec<DependencyNode>,
    /// Everything downstream, nearest first (the children included).
    pub downstream: Vec<DependencyNode>,
}

fn strings(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::String(s)) if !s.is_empty() => vec![s.clone()],
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

/// The parts of a resolved config the dialog shows.
pub fn build_config(config: &serde_json::Map<String, Value>) -> BuildConfig {
    let text = |key: &str| config.get(key).and_then(Value::as_str).map(str::to_string);
    let partition_by = match config.get("partition_by") {
        // BigQuery: {field: day, data_type: date, granularity: …}
        Some(Value::Object(dict)) => strings(dict.get("field")),
        other => strings(other),
    };
    BuildConfig {
        materialized: text("materialized"),
        incremental_strategy: text("incremental_strategy"),
        unique_key: strings(config.get("unique_key")),
        partition_by,
        cluster_by: strings(config.get("cluster_by")),
        on_schema_change: text("on_schema_change"),
    }
}

fn node_of(universe: &Universe, index: usize, depth: u32) -> DependencyNode {
    let node = &universe.nodes[index];
    // a test's config is how it fails, not how it is built: left out
    let config = if matches!(
        node.resource_type,
        ResourceType::Test | ResourceType::UnitTest
    ) {
        BuildConfig::default()
    } else {
        build_config(&node.config)
    };
    DependencyNode {
        name: node.display_name(),
        kind: node.resource_type.as_str(),
        package: node.package.clone(),
        uri: (!node.uri.as_os_str().is_empty()).then(|| file_uri(&node.uri)),
        line: node.range.start.line,
        depth,
        tags: node.tags.clone(),
        config,
    }
}

/// Breadth first along `edges` from `start`: every node once, at its
/// shortest distance, nearest first, then by name. Tests are left out
/// unless `tests` (they hang off every model and would bury the rest).
fn walk(
    universe: &Universe,
    edges: &[Vec<usize>],
    start: usize,
    tests: bool,
) -> Vec<DependencyNode> {
    let mut seen = vec![false; universe.nodes.len()];
    seen[start] = true;
    let mut queue = VecDeque::from([(start, 0u32)]);
    let mut out = Vec::new();
    while let Some((at, depth)) = queue.pop_front() {
        let mut next: Vec<usize> = edges[at].iter().copied().filter(|&n| !seen[n]).collect();
        next.sort_by(|a, b| universe.nodes[*a].name.cmp(&universe.nodes[*b].name));
        for n in next {
            seen[n] = true;
            let is_test = matches!(
                universe.nodes[n].resource_type,
                ResourceType::Test | ResourceType::UnitTest
            );
            if is_test && !tests {
                continue;
            }
            out.push(node_of(universe, n, depth + 1));
            queue.push_back((n, depth + 1));
        }
    }
    out
}

/// The dependencies of the model named `model` (a versioned model by its
/// name and, when given, its version), or `None` when it is not a node.
pub fn dependencies(
    universe: &Universe,
    model: &str,
    version: Option<&str>,
) -> Option<Dependencies> {
    let index = universe.nodes.iter().position(|n| {
        n.resource_type == ResourceType::Model
            && n.name == model
            && (version.is_none() || n.version_number.as_deref() == version)
    })?;
    let upstream = walk(universe, &universe.parents, index, false);
    let downstream = walk(universe, &universe.children, index, false);
    Some(Dependencies {
        model: node_of(universe, index, 0),
        parents: upstream.iter().filter(|n| n.depth == 1).cloned().collect(),
        children: downstream
            .iter()
            .filter(|n| n.depth == 1)
            .cloned()
            .collect(),
        upstream,
        downstream,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selectors::universe::Node;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    fn model(universe: &mut Universe, name: &str, config: Value) -> usize {
        let mut node = Node::new(ResourceType::Model, name, "p");
        node.config = config.as_object().cloned().unwrap_or_default();
        universe.push(node)
    }

    /// raw → stg → dim → fct ← (test on fct), and stg → other
    fn project() -> Universe {
        let mut u = Universe::default();
        let mut raw = Node::new(ResourceType::Source, "orders", "p");
        raw.source = Some(("raw".into(), "orders".into()));
        let raw = u.push(raw);
        let stg = model(&mut u, "stg", json!({"materialized": "view"}));
        let dim = model(
            &mut u,
            "dim",
            json!({"materialized": "incremental", "unique_key": ["id", "day"],
                   "partition_by": {"field": "day", "data_type": "date"},
                   "incremental_strategy": "merge"}),
        );
        let fct = model(
            &mut u,
            "fct",
            json!({"materialized": "table", "partition_by": "day"}),
        );
        let other = model(&mut u, "other", json!({}));
        let test = u.push(Node::new(ResourceType::Test, "unique_fct_id", "p"));
        u.link(stg, raw);
        u.link(dim, stg);
        u.link(fct, dim);
        u.link(other, stg);
        u.link(test, fct);
        u
    }

    fn names(nodes: &[DependencyNode]) -> Vec<(String, u32)> {
        nodes.iter().map(|n| (n.name.clone(), n.depth)).collect()
    }

    #[test]
    fn direct_and_transitive_both_ways() {
        let deps = dependencies(&project(), "dim", None).unwrap();
        assert_eq!(names(&deps.parents), vec![("stg".into(), 1)]);
        assert_eq!(
            names(&deps.upstream),
            vec![("stg".into(), 1), ("raw.orders".into(), 2)]
        );
        assert_eq!(names(&deps.children), vec![("fct".into(), 1)]);
        // the test on fct is left out
        assert_eq!(names(&deps.downstream), vec![("fct".into(), 1)]);
        let stg = dependencies(&project(), "stg", None).unwrap();
        assert_eq!(
            names(&stg.children),
            vec![("dim".into(), 1), ("other".into(), 1)]
        );
        assert_eq!(
            names(&stg.downstream),
            vec![("dim".into(), 1), ("other".into(), 1), ("fct".into(), 2)]
        );
    }

    #[test]
    fn an_incremental_model_says_its_keys() {
        let deps = dependencies(&project(), "dim", None).unwrap();
        assert_eq!(
            deps.model.config,
            BuildConfig {
                materialized: Some("incremental".into()),
                incremental_strategy: Some("merge".into()),
                unique_key: vec!["id".into(), "day".into()],
                partition_by: vec!["day".into()],
                cluster_by: vec![],
                on_schema_change: None,
            }
        );
        assert_eq!(
            deps.children[0].config.partition_by,
            vec!["day".to_string()]
        );
        assert_eq!(deps.upstream[1].kind, "source");
    }

    #[test]
    fn a_name_that_is_no_model() {
        assert!(dependencies(&project(), "nope", None).is_none());
        assert!(dependencies(&project(), "unique_fct_id", None).is_none());
    }

    #[test]
    fn a_cycle_ends() {
        let mut u = project();
        let dim = u.find(ResourceType::Model, "dim").unwrap();
        let fct = u.find(ResourceType::Model, "fct").unwrap();
        u.link(dim, fct);
        let deps = dependencies(&u, "dim", None).unwrap();
        assert!(deps.downstream.iter().all(|n| n.name != "dim"));
        assert!(deps.upstream.iter().any(|n| n.name == "fct"));
    }
}
