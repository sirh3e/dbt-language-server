//! `dbt.columnLineageGraph`: the column lineage around one model column,
//! walked across the project in one answer, over the project's
//! [`ColumnGraph`]. Pure: the shell builds the graph from the lineages it
//! has and says which models it could not read; the walk reads no file.
//!
//! What the graph decides holds here too: column names match ignoring
//! case, a `*` is resolved into the columns it passes on, dbt's
//! `__dbt_rel_*` placeholders are no columns. The payload is the one the
//! Neovim plugin draws: node ids `model:<m>::<col>`, `source:<s>.<t>::<col>`,
//! `unknown:<name>::<col>`, the start's columns first, then the upstream
//! walk's, then the downstream walk's, each breadth first; edges once each,
//! `kind` `direct` (passed on unchanged, by name or by a `*`) or `derived`;
//! each node's `layer` as `dag_data.relayer` gives it.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use super::ProjectGraph;
use crate::lineage::graph::NodeIndex;
use crate::lineage::{ColumnGraph, Flow, Relation};

mod action;
mod layers;
pub mod text;

pub use action::{show_lineage_action, SHOW_COMMAND};
#[cfg(test)]
mod tests;

/// Which way to walk from the start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Both,
}

impl Direction {
    /// The wire word; anything else is `None`.
    pub fn parse(word: &str) -> Option<Self> {
        Some(match word {
            "up" => Direction::Up,
            "down" => Direction::Down,
            "both" => Direction::Both,
            _ => return None,
        })
    }
    fn up(self) -> bool {
        self != Direction::Down
    }
    fn down(self) -> bool {
        self != Direction::Up
    }
}

/// One column on the wire.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LineageNode {
    pub id: String,
    pub name: String,
    pub kind: &'static str,
    /// `null` when the owner has no file.
    pub uri: Option<String>,
    pub tags: Vec<String>,
    pub columns: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub column: String,
    pub layer: usize,
    #[serde(rename = "compiledUri", skip_serializing_if = "Option::is_none")]
    pub compiled_uri: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LineageEdge {
    pub from: String,
    pub to: String,
    pub kind: &'static str,
}

/// What the walk found.
#[derive(Debug, Default, PartialEq, Serialize)]
pub struct WalkResult {
    pub nodes: Vec<LineageNode>,
    pub edges: Vec<LineageEdge>,
    /// The node to select first.
    pub start: Option<String>,
    /// Models whose lineage could not be read (declined, unparsable).
    pub missing: Vec<String>,
    /// Models whose lineage is not computed yet: the graph is without them.
    pub stale: Vec<String>,
}

/// Where the walk starts and how far it goes.
pub struct WalkInput<'a> {
    pub project: &'a ProjectGraph,
    /// The column graph of the models within `depth` of the start.
    pub graph: &'a ColumnGraph,
    pub model: &'a str,
    pub column: Option<&'a str>,
    pub direction: Direction,
    /// Hops each way, at least 1.
    pub depth: usize,
    /// A model's compiled file, as a uri, when one is on disk.
    pub compiled_uri: &'a dyn Fn(&str) -> Option<String>,
    /// Models whose lineage declined; models not computed yet.
    pub missing: Vec<String>,
    pub stale: Vec<String>,
}

/// The lineage around `input`'s start, up and down `depth` hops.
pub fn walk(input: &WalkInput<'_>) -> WalkResult {
    let graph = input.graph;
    let depth = Some(input.depth.max(1));
    let starts: Vec<NodeIndex> = match input.column {
        Some(column) => graph
            .find_model_column(input.model, column)
            .into_iter()
            .collect(),
        None => graph.outputs(input.model).to_vec(),
    };
    let mut wire = Wire::new(input);
    for &node in &starts {
        wire.node(node);
    }
    // a column the graph does not hold (nothing reads it, a declined
    // template) still gets its node
    let start = match (starts.first(), input.column) {
        (Some(&first), _) => Some(wire.id_of(first)),
        (None, Some(column)) => Some(wire.model_column(input.model, column)),
        (None, None) => None,
    };
    if input.direction.up() {
        let reached: Vec<NodeIndex> = graph
            .upstream(&starts, depth)
            .iter()
            .map(|r| r.node)
            .collect();
        let within: HashSet<NodeIndex> = starts.iter().chain(&reached).copied().collect();
        for &node in &reached {
            wire.node(node);
        }
        for &node in starts.iter().chain(&reached) {
            for (from, flow) in graph.reads(node) {
                if within.contains(&from) {
                    wire.edge(from, node, flow);
                }
            }
        }
    }
    if input.direction.down() {
        let reached: Vec<NodeIndex> = graph
            .downstream(&starts, depth)
            .iter()
            .map(|r| r.node)
            .collect();
        let within: HashSet<NodeIndex> = starts.iter().chain(&reached).copied().collect();
        for &node in &reached {
            wire.node(node);
        }
        for &node in starts.iter().chain(&reached) {
            for (to, flow) in graph.read_by(node) {
                if within.contains(&to) {
                    wire.edge(node, to, flow);
                }
            }
        }
    }
    wire.finish(start)
}

/// The answer as it goes on the wire.
struct Wire<'a> {
    input: &'a WalkInput<'a>,
    uris: HashMap<&'a str, Option<&'a str>>,
    nodes: Vec<LineageNode>,
    seen_nodes: HashSet<String>,
    edges: Vec<LineageEdge>,
    seen_edges: HashSet<(String, String)>,
}

impl<'a> Wire<'a> {
    fn new(input: &'a WalkInput<'a>) -> Self {
        let uris = input
            .project
            .nodes
            .iter()
            .map(|n| (n.id.as_str(), n.uri.as_deref()))
            .collect();
        Wire {
            input,
            uris,
            nodes: Vec::new(),
            seen_nodes: HashSet::new(),
            edges: Vec::new(),
            seen_edges: HashSet::new(),
        }
    }

    fn uri_of(&self, id: &str) -> Option<String> {
        self.uris.get(id).copied().flatten().map(str::to_string)
    }

    /// The wire id of a graph column, the node added on first sight.
    fn id_of(&mut self, node: NodeIndex) -> String {
        let column = self.input.graph.column(node);
        let name = column.column.clone();
        match column.relation.clone() {
            Relation::Model(model) => self.model_column(&model, &name),
            Relation::Source { source, table } => {
                let uri = self.uri_of(&format!("source:{source}"));
                self.add(LineageNode {
                    id: format!("source:{source}.{table}::{name}"),
                    name: format!("{source}.{table}.{name}"),
                    kind: "source",
                    uri,
                    tags: Vec::new(),
                    columns: Vec::new(),
                    model: None,
                    column: name,
                    layer: 0,
                    compiled_uri: None,
                })
            }
            Relation::Unknown(relation) => self.add(LineageNode {
                id: format!("unknown:{relation}::{name}"),
                name: format!("{relation}.{name}"),
                kind: "unknown",
                uri: None,
                tags: Vec::new(),
                columns: Vec::new(),
                model: None,
                column: name,
                layer: 0,
                compiled_uri: None,
            }),
        }
    }

    fn node(&mut self, node: NodeIndex) {
        self.id_of(node);
    }

    fn model_column(&mut self, model: &str, column: &str) -> String {
        let uri = self.uri_of(&format!("model:{model}"));
        let compiled_uri = (self.input.compiled_uri)(model);
        self.add(LineageNode {
            id: format!("model:{model}::{column}"),
            name: format!("{model}.{column}"),
            kind: "model",
            uri,
            tags: Vec::new(),
            columns: Vec::new(),
            model: Some(model.to_string()),
            column: column.to_string(),
            layer: 0,
            compiled_uri,
        })
    }

    /// Adds `node` unless its id is there; its id either way.
    fn add(&mut self, node: LineageNode) -> String {
        let id = node.id.clone();
        if self.seen_nodes.insert(id.clone()) {
            self.nodes.push(node);
        }
        id
    }

    fn edge(&mut self, from: NodeIndex, to: NodeIndex, flow: Flow) {
        let (from, to) = (self.id_of(from), self.id_of(to));
        let kind = match flow {
            Flow::Derived => "derived",
            Flow::Direct | Flow::Star => "direct",
        };
        if self.seen_edges.insert((from.clone(), to.clone())) {
            self.edges.push(LineageEdge { from, to, kind });
        }
    }

    fn finish(mut self, start: Option<String>) -> WalkResult {
        layers::assign(&mut self.nodes, &self.edges);
        WalkResult {
            nodes: self.nodes,
            edges: self.edges,
            start,
            missing: self.input.missing.clone(),
            stale: self.input.stale.clone(),
        }
    }
}
