use std::collections::HashMap;
use std::sync::Arc;

use super::*;
use crate::features::graph_nav::{ProjectGraphEdge, ProjectGraphNode};
use crate::lineage::ColumnLineage;

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

fn run(direction: Direction, column: Option<&str>, pending: &[&str]) -> WalkResult {
    let project = project();
    let lineages = lineages();
    let none = |_: &str| None;
    let input = WalkInput {
        project: &project,
        model: "stg",
        column,
        direction,
        depth: 3,
        compiled_uri: &none,
    };
    walk(&input, &|m| {
        if pending.contains(&m) {
            Known::Pending
        } else {
            Known::Lineage(lineages[m].clone())
        }
    })
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
