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
use crate::features::column_types::{Basis, ModelColumnTypes};
use crate::features::file_uri;

/// A model's output columns with their types, by its name, or none
/// before they are known (the shell composes the schema snapshot's
/// inference and the YAML's `data_type`s: [`columns_of`],
/// [`with_yaml_types`]).
/// Its second argument says whether the node's resolved config enforces
/// its contract (`dbt_project.yml`'s `+contract` included).
pub type Types<'a> = &'a dyn Fn(&str, bool) -> Option<Vec<DependencyColumn>>;

/// One output column and its type, as the dialog shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyColumn {
    pub name: String,
    /// The dialect's spelling, when the type is pinned exactly.
    pub data_type: Option<String>,
    /// For people: the spelling, or the family and what is unproven.
    pub description: String,
    pub nullable: Option<bool>,
    pub basis: Basis,
    /// One line on why the type is what it is.
    pub why: String,
}

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
    /// A model's output columns with their types; `None` when they are
    /// not inferred (yet) or the node is no model.
    pub columns: Option<Vec<DependencyColumn>>,
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

/// The dialog's columns of an inference.
pub fn columns_of(types: &ModelColumnTypes) -> Vec<DependencyColumn> {
    types
        .columns
        .iter()
        .map(|c| DependencyColumn {
            name: c.name.clone(),
            data_type: c.data_type.clone(),
            description: c.description.clone(),
            nullable: c.nullable,
            basis: c.basis,
            why: c.why.clone(),
        })
        .collect()
}

/// The YAML's `data_type`s over what inference left unknown: a column
/// inference could not type takes the one written for it, and a model it
/// could not plan at all takes its YAML columns. Proven under an enforced
/// contract (dbt checks it at build time), declared otherwise.
pub fn with_yaml_types(
    inferred: Option<Vec<DependencyColumn>>,
    yaml: &[cogitator_schemas::ColumnEntry],
    contract_enforced: bool,
) -> Option<Vec<DependencyColumn>> {
    let basis = if contract_enforced {
        Basis::Proven
    } else {
        Basis::Declared
    };
    let why = if contract_enforced {
        "its YAML data_type, under an enforced contract"
    } else {
        "its YAML data_type, not enforced"
    };
    let typed = |c: &cogitator_schemas::ColumnEntry| {
        c.data_type
            .as_ref()
            .filter(|t| !t.trim().is_empty())
            .map(|t| DependencyColumn {
                name: c.name.clone(),
                data_type: Some(t.clone()),
                description: t.clone(),
                nullable: None,
                basis,
                why: why.to_string(),
            })
    };
    match inferred {
        Some(mut columns) if !columns.is_empty() => {
            for column in &mut columns {
                if column.basis != Basis::Unknown {
                    continue;
                }
                if let Some(entry) = yaml
                    .iter()
                    .find(|c| c.name.eq_ignore_ascii_case(&column.name))
                {
                    if let Some(from_yaml) = typed(entry) {
                        *column = DependencyColumn {
                            nullable: column.nullable,
                            ..from_yaml
                        };
                    }
                }
            }
            Some(columns)
        }
        inferred => {
            let from_yaml: Vec<DependencyColumn> = yaml.iter().filter_map(typed).collect();
            if from_yaml.is_empty() {
                inferred
            } else {
                Some(from_yaml)
            }
        }
    }
}

fn node_of(universe: &Universe, index: usize, depth: u32, types: Types<'_>) -> DependencyNode {
    let node = &universe.nodes[index];
    let enforced = node
        .config
        .get("contract")
        .and_then(|c| c.get("enforced"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    // by name, else by the file's stem (a versioned model's file)
    let columns = (node.resource_type == ResourceType::Model)
        .then(|| {
            types(&node.name, enforced).or_else(|| {
                node.uri
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .filter(|stem| *stem != node.name)
                    .and_then(|stem| types(stem, enforced))
            })
        })
        .flatten();
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
        columns,
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
    types: Types<'_>,
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
            out.push(node_of(universe, n, depth + 1, types));
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
    dependencies_with(universe, model, version, &|_, _| None)
}

/// [`dependencies`] with each model's output-column types from `types`.
pub fn dependencies_with(
    universe: &Universe,
    model: &str,
    version: Option<&str>,
    types: Types<'_>,
) -> Option<Dependencies> {
    let index = universe.nodes.iter().position(|n| {
        n.resource_type == ResourceType::Model
            && n.name == model
            && (version.is_none() || n.version_number.as_deref() == version)
    })?;
    let upstream = walk(universe, &universe.parents, index, false, types);
    let downstream = walk(universe, &universe.children, index, false, types);
    Some(Dependencies {
        model: node_of(universe, index, 0, types),
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
    fn models_carry_their_column_types() {
        use crate::features::column_types::{ColumnType, ModelColumnTypes, Provenance};
        let types = |model: &str, _enforced: bool| {
            (model == "dim" || model == "stg").then(|| {
                columns_of(&ModelColumnTypes {
                    model: model.to_string(),
                    revision: 1,
                    columns: vec![ColumnType {
                        name: "id".into(),
                        data_type: Some("BIGINT".into()),
                        description: "BIGINT".into(),
                        nullable: Some(false),
                        basis: Basis::Proven,
                        why: "inferred from stg.id".into(),
                        sql_type: cogitator_ir::SqlType::Unknown,
                        provenance: Provenance::Cast,
                        feeds: Vec::new(),
                    }],
                })
            })
        };
        let deps = dependencies_with(&project(), "dim", None, &types).unwrap();
        let id = &deps.model.columns.as_ref().unwrap()[0];
        assert_eq!(
            (
                id.name.as_str(),
                id.data_type.as_deref(),
                id.nullable,
                id.basis
            ),
            ("id", Some("BIGINT"), Some(false), Basis::Proven)
        );
        // stg has types, the source none, fct none yet
        assert!(deps.parents[0].columns.is_some());
        assert!(deps.upstream[1].columns.is_none());
        assert!(deps.children[0].columns.is_none());
        // without a lookup nothing is claimed
        assert!(dependencies(&project(), "dim", None)
            .unwrap()
            .model
            .columns
            .is_none());
    }

    fn yaml(name: &str, data_type: Option<&str>) -> cogitator_schemas::ColumnEntry {
        cogitator_schemas::ColumnEntry {
            name: name.into(),
            description: String::new(),
            name_pos: Default::default(),
            data_type: data_type.map(str::to_string),
            tests: Vec::new(),
            constraints: Vec::new(),
        }
    }

    fn column(name: &str, data_type: Option<&str>, basis: Basis) -> DependencyColumn {
        DependencyColumn {
            name: name.into(),
            data_type: data_type.map(str::to_string),
            description: data_type.unwrap_or("unknown").into(),
            nullable: None,
            basis,
            why: String::new(),
        }
    }

    #[test]
    fn the_yaml_types_fill_what_inference_left_unknown() {
        let entries = [
            yaml("id", Some("bigint")),
            yaml("day", Some("date")),
            yaml("note", None),
        ];
        // inference typed `id`, not `day`: day takes its YAML type
        let got = with_yaml_types(
            Some(vec![
                column("id", Some("BIGINT"), Basis::Proven),
                column("day", None, Basis::Unknown),
            ]),
            &entries,
            true,
        )
        .unwrap();
        assert_eq!(
            got.iter()
                .map(|c| (c.name.as_str(), c.data_type.as_deref(), c.basis))
                .collect::<Vec<_>>(),
            vec![
                ("id", Some("BIGINT"), Basis::Proven),
                ("day", Some("date"), Basis::Proven)
            ]
        );
        // nothing inferred: the YAML's typed columns, declared without a contract
        let got = with_yaml_types(None, &entries, false).unwrap();
        assert_eq!(got.len(), 2);
        assert!(got.iter().all(|c| c.basis == Basis::Declared));
        // nothing anywhere stays nothing
        assert!(with_yaml_types(None, &[yaml("x", None)], true).is_none());
        assert_eq!(with_yaml_types(Some(vec![]), &[], false), Some(vec![]));
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
