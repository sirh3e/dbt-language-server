//! Project dependency graph on petgraph (specs/rust-port/05 §5).
//!
//! Built during context refresh by running the live-document pipeline over
//! every model file and recording REF/SOURCE/MACRO tokens as edges. petgraph
//! types never cross this module's boundary.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use cogitator_dialects::Dialect;
use cogitator_parser::{DocumentParser, JinjaSqlParser, TokenKind};
use cogitator_schemas::project::ProjectFile;
use petgraph::stable_graph::{NodeIndex, StableDiGraph};

use crate::util;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeKind {
    Model,
    Source,
    Macro,
}

#[derive(Debug, Clone)]
pub struct NodeData {
    pub kind: NodeKind,
    pub name: String,
    pub uri: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeKind {
    Ref,
    Source,
    MacroCall,
}

#[derive(Debug, Default)]
pub struct ProjectGraph {
    graph: StableDiGraph<NodeData, EdgeKind>,
    by_name: HashMap<(NodeKind, String), NodeIndex>,
}

impl ProjectGraph {
    pub fn build(root: &Path, project: &ProjectFile, dialect: &Dialect) -> Self {
        let mut graph = ProjectGraph::default();
        let parser = JinjaSqlParser;
        let model_files = util::create_file_name_map(&[".sql"], root, &project.model_paths);

        for (name, path) in &model_files {
            graph.ensure_node(NodeKind::Model, name.clone(), path.clone());
        }
        for (name, path) in &model_files {
            let Ok(text) = std::fs::read_to_string(path) else {
                continue;
            };
            let Ok(doc) = parser.parse(&text, dialect) else {
                continue;
            };
            let from = graph.ensure_node(NodeKind::Model, name.clone(), path.clone());
            for token in &doc.tokens {
                let (kind, edge) = match token.kind {
                    TokenKind::Ref => (NodeKind::Model, EdgeKind::Ref),
                    TokenKind::SourceName => (NodeKind::Source, EdgeKind::Source),
                    TokenKind::Macro => (NodeKind::Macro, EdgeKind::MacroCall),
                    _ => continue,
                };
                let target = doc.literal(token).to_string();
                let to = graph.ensure_node(kind, target, PathBuf::new());
                graph.graph.update_edge(from, to, edge);
            }
        }
        graph
    }

    fn ensure_node(&mut self, kind: NodeKind, name: String, uri: PathBuf) -> NodeIndex {
        match self.by_name.get(&(kind, name.clone())) {
            Some(&idx) => idx,
            None => {
                let idx = self.graph.add_node(NodeData {
                    kind,
                    name: name.clone(),
                    uri,
                });
                self.by_name.insert((kind, name), idx);
                idx
            }
        }
    }

    /// Models whose refs point at nothing on disk (05 §5 use 2).
    pub fn dangling_refs(&self) -> impl Iterator<Item = (&NodeData, &NodeData)> {
        self.graph.edge_indices().filter_map(move |e| {
            let (from, to) = self.graph.edge_endpoints(e)?;
            let (fd, td) = (&self.graph[from], &self.graph[to]);
            (matches!(self.graph[e], EdgeKind::Ref)
                && td.kind == NodeKind::Model
                && td.uri.as_os_str().is_empty())
            .then_some((fd, td))
        })
    }

    /// Incoming `Ref` edges of a model (05 §5 use 3: find-references).
    pub fn dependents_of<'a>(&'a self, name: &str) -> impl Iterator<Item = &'a NodeData> + 'a {
        let target = self
            .by_name
            .get(&(NodeKind::Model, name.to_string()))
            .copied();
        target.into_iter().flat_map(move |idx| {
            self.graph
                .neighbors_directed(idx, petgraph::Direction::Incoming)
                .map(|n| &self.graph[n])
        })
    }

    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Deterministic edge list for snapshots (05 §7).
    pub fn sorted_edges(&self) -> Vec<(String, &'static str, String)> {
        let mut edges: Vec<_> = self
            .graph
            .edge_indices()
            .filter_map(|e| {
                let (from, to) = self.graph.edge_endpoints(e)?;
                let kind = match self.graph[e] {
                    EdgeKind::Ref => "ref",
                    EdgeKind::Source => "source",
                    EdgeKind::MacroCall => "macro",
                };
                Some((
                    self.graph[from].name.clone(),
                    kind,
                    self.graph[to].name.clone(),
                ))
            })
            .collect();
        edges.sort();
        edges
    }
}
