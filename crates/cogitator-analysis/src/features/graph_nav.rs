//! dbt.modelGraph / dbt.projectGraph: the graph payloads behind the editor
//! browsers (editors/nvim graph.lua, browse.lua and dag.lua), as typed wire
//! models. `uri` fields are `None` → `null` on the wire (dangling refs),
//! never omitted.

use serde::Serialize;

use crate::graph::{NodeData, NodeKind};

use super::{file_uri, model_name_from_uri, DbtContext};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GraphNodeKind {
    Model,
    Source,
}

impl GraphNodeKind {
    fn of(kind: NodeKind) -> Self {
        if kind == NodeKind::Source {
            GraphNodeKind::Source
        } else {
            GraphNodeKind::Model
        }
    }
}

/// One neighbor entry, shared by dbt.modelGraph and dbt.modelInfo so their
/// wire shapes cannot drift.
#[derive(Debug, Serialize)]
pub struct GraphEntry {
    pub name: String,
    pub kind: GraphNodeKind,
    pub uri: Option<String>,
}

/// The dbt.modelGraph result.
#[derive(Debug, Serialize)]
pub struct ModelGraph {
    pub model: String,
    pub uri: String,
    pub upstream: Vec<GraphEntry>,
    pub downstream: Vec<GraphEntry>,
}

#[derive(Debug, Serialize)]
pub struct GraphColumn {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Serialize)]
pub struct ProjectGraphNode {
    pub id: String,
    pub name: String,
    pub kind: GraphNodeKind,
    pub layer: usize,
    pub uri: Option<String>,
    pub tags: Vec<String>,
    pub columns: Vec<GraphColumn>,
}

#[derive(Debug, Serialize)]
pub struct ProjectGraphEdge {
    pub from: String,
    pub to: String,
}

/// The dbt.projectGraph result.
#[derive(Debug, Serialize)]
pub struct ProjectGraph {
    pub nodes: Vec<ProjectGraphNode>,
    pub edges: Vec<ProjectGraphEdge>,
}

pub(crate) fn graph_entry(ctx: &DbtContext, node: &NodeData) -> GraphEntry {
    let uri = match node.kind {
        NodeKind::Source => ctx
            .source_detail_map
            .get(&node.name)
            .map(|s| file_uri(&s.uri)),
        _ => (!node.uri.as_os_str().is_empty()).then(|| file_uri(&node.uri)),
    };
    GraphEntry {
        name: node.name.clone(),
        kind: GraphNodeKind::of(node.kind),
        uri,
    }
}

/// The model, its file, and its direct upstream/downstream neighbors with
/// navigable URIs. `model_arg` (re-centering on a neighbor) wins over `uri`.
pub fn model_graph(ctx: &DbtContext, uri: &str, model_arg: Option<&str>) -> Option<ModelGraph> {
    let model = match model_arg {
        Some(m) if !m.is_empty() => m.to_string(),
        _ => model_name_from_uri(uri)?,
    };
    let details = ctx.model_detail_map.get(&model)?;
    let (upstream, downstream) = ctx.graph.model_neighbors(&model);
    Some(ModelGraph {
        uri: file_uri(&details.uri),
        upstream: upstream.into_iter().map(|n| graph_entry(ctx, n)).collect(),
        downstream: downstream
            .into_iter()
            .map(|n| graph_entry(ctx, n))
            .collect(),
        model,
    })
}

fn node_id(kind: NodeKind, name: &str) -> String {
    format!(
        "{}:{name}",
        if kind == NodeKind::Source {
            "source"
        } else {
            "model"
        }
    )
}

/// The whole model/source DAG with layers, tags and documented columns.
pub fn project_graph_json(ctx: &DbtContext) -> ProjectGraph {
    let (nodes, edges) = ctx.graph.dag();
    let nodes = nodes
        .iter()
        .map(|(data, layer)| {
            let (uri, tags, columns) = match data.kind {
                NodeKind::Source => (
                    ctx.source_detail_map
                        .get(&data.name)
                        .map(|s| file_uri(&s.uri)),
                    Vec::new(),
                    Vec::new(),
                ),
                _ => {
                    let details = ctx.model_detail_map.get(&data.name);
                    (
                        details
                            .map(|d| &d.uri)
                            .filter(|p| !p.as_os_str().is_empty())
                            .map(|p| file_uri(p)),
                        details.map(|d| d.tags.clone()).unwrap_or_default(),
                        details
                            .map(|d| {
                                d.columns
                                    .iter()
                                    .map(|c| GraphColumn {
                                        name: c.name.clone(),
                                        description: c.description.clone(),
                                    })
                                    .collect()
                            })
                            .unwrap_or_default(),
                    )
                }
            };
            ProjectGraphNode {
                id: node_id(data.kind, &data.name),
                name: data.name.clone(),
                kind: GraphNodeKind::of(data.kind),
                layer: *layer,
                uri,
                tags,
                columns,
            }
        })
        .collect();
    let edges = edges
        .iter()
        .map(|((fk, fname), (tk, tname))| ProjectGraphEdge {
            from: node_id(*fk, fname),
            to: node_id(*tk, tname),
        })
        .collect();
    ProjectGraph { nodes, edges }
}
