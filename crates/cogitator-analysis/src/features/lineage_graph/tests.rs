use std::collections::HashMap;
use std::sync::Arc;

use super::*;
use crate::features::graph_nav::GraphNodeKind;
use crate::features::graph_nav::{ProjectGraphEdge, ProjectGraphNode};
use crate::lineage::SourceRef;
use crate::lineage::{ColumnLineage, ModelLineage};

fn node(id: &str, kind: GraphNodeKind) -> ProjectGraphNode {
    ProjectGraphNode {
        id: id.to_string(),
        name: id.split_once(':').unwrap().1.to_string(),
        kind,
        layer: 0,
        uri: Some(format!("file:///{id}")),
        tags: Vec::new(),
        columns: Vec::new(),
    }
}

/// raw ← stg ← (a, b); stg reads a source too.
fn project() -> ProjectGraph {
    let edge = |from: &str, to: &str| ProjectGraphEdge {
        from: from.to_string(),
        to: to.to_string(),
    };
    ProjectGraph {
        nodes: vec![
            node("source:shop", GraphNodeKind::Source),
            node("model:stg", GraphNodeKind::Model),
            node("model:a", GraphNodeKind::Model),
            node("model:b", GraphNodeKind::Model),
        ],
        edges: vec![
            edge("source:shop", "model:stg"),
            edge("model:stg", "model:a"),
            edge("model:stg", "model:b"),
        ],
    }
}

fn col(name: &str, direct: bool, sources: Vec<SourceRef>) -> ColumnLineage {
    ColumnLineage {
        name: name.to_string(),
        sources,
        direct,
        derived: false,
        end: None,
    }
}

fn model(name: &str, column: &str) -> SourceRef {
    SourceRef::Model {
        name: name.to_string(),
        column: column.to_string(),
    }
}

fn lineages() -> HashMap<&'static str, Arc<Option<ModelLineage>>> {
    let some = |columns| {
        Arc::new(Some(ModelLineage {
            columns,
            origin: None,
        }))
    };
    let shop = SourceRef::Source {
        source: "shop".into(),
        table: "customers".into(),
        column: "id".into(),
    };
    HashMap::from([
        ("stg", some(vec![col("customer_id", false, vec![shop])])),
        (
            "a",
            some(vec![col(
                "customer_id",
                true,
                vec![model("stg", "customer_id")],
            )]),
        ),
        ("b", Arc::new(None)),
    ])
}

/// What the shell does: the graph of the lineages it has, the declined
/// ones missing, the pending ones stale.
fn run(direction: Direction, column: Option<&str>, pending: &[&str]) -> WalkResult {
    walk_over(&lineages(), direction, column, pending)
}

fn walk_over(
    lineages: &HashMap<&'static str, Arc<Option<ModelLineage>>>,
    direction: Direction,
    column: Option<&str>,
    pending: &[&str],
) -> WalkResult {
    let project = project();
    let mut models: Vec<&&str> = lineages.keys().collect();
    models.sort();
    let (mut known, mut missing, mut stale) = (Vec::new(), Vec::new(), Vec::new());
    for m in models {
        match (pending.contains(m), lineages[m].as_ref()) {
            (true, _) => stale.push(m.to_string()),
            (false, Some(l)) => known.push((*m, l)),
            (false, None) => missing.push(m.to_string()),
        }
    }
    let graph = ColumnGraph::build(known);
    let none = |_: &str| None;
    let input = WalkInput {
        project: &project,
        graph: &graph,
        model: "stg",
        column,
        direction,
        depth: 3,
        compiled_uri: &none,
        missing,
        stale,
    };
    walk(&input)
}

fn ids(result: &WalkResult) -> Vec<&str> {
    result.nodes.iter().map(|n| n.id.as_str()).collect()
}

#[test]
fn both_ways_meets_the_source_and_the_consumer() {
    let result = run(Direction::Both, Some("customer_id"), &[]);
    assert_eq!(
        ids(&result),
        [
            "model:stg::customer_id",
            "source:shop.customers::id",
            "model:a::customer_id",
        ]
    );
    let edges: Vec<_> = result
        .edges
        .iter()
        .map(|e| (e.from.as_str(), e.to.as_str(), e.kind))
        .collect();
    assert_eq!(
        edges,
        [
            (
                "source:shop.customers::id",
                "model:stg::customer_id",
                "derived"
            ),
            ("model:stg::customer_id", "model:a::customer_id", "direct"),
        ]
    );
    assert_eq!(result.start.as_deref(), Some("model:stg::customer_id"));
    assert_eq!(result.missing, ["b"], "a declined lineage is missing");
    let layers: Vec<_> = result.nodes.iter().map(|n| n.layer).collect();
    assert_eq!(layers, [1, 0, 2]);
    assert_eq!(result.nodes[1].uri.as_deref(), Some("file:///source:shop"));
}

#[test]
fn one_direction_leaves_the_other_out() {
    let up = run(Direction::Up, Some("customer_id"), &[]);
    assert_eq!(
        ids(&up),
        ["model:stg::customer_id", "source:shop.customers::id"]
    );
    let down = run(Direction::Down, Some("customer_id"), &[]);
    assert_eq!(
        ids(&down),
        ["model:stg::customer_id", "model:a::customer_id"]
    );
}

#[test]
fn a_pending_model_is_stale_not_missing() {
    let result = run(Direction::Down, Some("customer_id"), &["a"]);
    assert_eq!(result.stale, ["a"]);
    assert!(result.missing.contains(&"b".to_string()));
    assert_eq!(ids(&result), ["model:stg::customer_id"]);
}

#[test]
fn an_unlisted_column_still_gets_its_node() {
    let result = run(Direction::Both, Some("nope"), &[]);
    assert_eq!(ids(&result), ["model:stg::nope"]);
    assert_eq!(result.start.as_deref(), Some("model:stg::nope"));
}

#[test]
fn the_wire_shape_is_the_plugins() {
    let result = run(Direction::Up, Some("customer_id"), &[]);
    let json = serde_json::to_value(&result.nodes).unwrap();
    assert_eq!(
        json[0],
        serde_json::json!({
            "id": "model:stg::customer_id", "name": "stg.customer_id", "kind": "model",
            "uri": "file:///model:stg", "tags": [], "columns": [], "model": "stg",
            "column": "customer_id", "layer": 1,
        })
    );
    assert!(json[1].get("model").is_none());
}

/// The graph's rules hold on the wire: a consumer naming the column in
/// another case reads it, and a `select *` passes it on under its name.
#[test]
fn case_and_stars_reach_the_consumers() {
    let mut lineages = lineages();
    let some = |columns| {
        Arc::new(Some(ModelLineage {
            columns,
            origin: None,
        }))
    };
    lineages.insert(
        "a",
        some(vec![col(
            "CUSTOMER_ID",
            true,
            vec![model("stg", "Customer_Id")],
        )]),
    );
    lineages.insert("b", some(vec![col("*", false, vec![model("stg", "*")])]));
    let down = walk_over(&lineages, Direction::Down, Some("customer_id"), &[]);
    assert_eq!(
        ids(&down),
        [
            "model:stg::customer_id",
            "model:a::CUSTOMER_ID",
            "model:b::customer_id",
        ]
    );
    assert!(
        down.edges.iter().all(|e| e.kind == "direct"),
        "{:?}",
        down.edges
    );
}
