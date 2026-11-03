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

use crate::cache::FileCache;
use crate::util;

mod traverse;

/// Cached per-model edge extraction result: (target kind, edge, target name).
pub type EdgeSeed = (NodeKind, EdgeKind, String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NodeKind {
    Model,
    Source,
    Macro,
    Exposure,
    Metric,
    SemanticModel,
}

impl NodeKind {
    /// Wire/debug name, stable across the dag payloads.
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeKind::Model => "model",
            NodeKind::Source => "source",
            NodeKind::Macro => "macro",
            NodeKind::Exposure => "exposure",
            NodeKind::Metric => "metric",
            NodeKind::SemanticModel => "semantic_model",
        }
    }
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
    /// Memoized DAG depths: the graph is immutable after build, and
    /// dbt.projectGraph used to recompute every layer on every call.
    depths: std::sync::OnceLock<HashMap<NodeIndex, usize>>,
}

impl ProjectGraph {
    pub fn build(
        root: &Path,
        project: &ProjectFile,
        dialect: &Dialect,
        cache: &mut FileCache<Vec<EdgeSeed>>,
    ) -> Self {
        let mut graph = ProjectGraph::default();
        let model_files = util::create_file_name_map(&[".sql"], root, &project.model_paths);

        for (name, path) in &model_files {
            graph.ensure_node(NodeKind::Model, name.clone(), path.clone());
        }
        for (name, path) in &model_files {
            let Some(seeds) =
                cache.get_or_parse(path, |text, _| Some(extract_edge_seeds(text, dialect)))
            else {
                continue;
            };
            let from = graph.ensure_node(NodeKind::Model, name.clone(), path.clone());
            for (kind, edge, target) in seeds.iter() {
                let to = graph.ensure_node(*kind, target.clone(), PathBuf::new());
                graph.graph.update_edge(from, to, *edge);
            }
        }
        cache.sweep();
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

    /// Models that depend on `name` — its *downstream* (incoming `Ref`
    /// edges; dbt terminology, 05 §5 use 3: find-references).
    pub fn downstream_of<'a>(&'a self, name: &str) -> impl Iterator<Item = &'a NodeData> + 'a {
        self.neighbors(NodeKind::Model, name, petgraph::Direction::Incoming)
    }

    /// What `name` depends on — its *upstream* (outgoing ref/source edges).
    pub fn upstream_of<'a>(&'a self, name: &str) -> impl Iterator<Item = &'a NodeData> + 'a {
        self.neighbors(NodeKind::Model, name, petgraph::Direction::Outgoing)
    }

    /// Direct neighbors for the model-graph browser (`dbt.modelGraph`):
    /// upstream = outgoing ref/source targets (macros excluded), downstream
    /// = incoming ref dependents. Sorted by name for a stable UI.
    pub fn model_neighbors<'a>(&'a self, name: &str) -> (Vec<&'a NodeData>, Vec<&'a NodeData>) {
        use petgraph::visit::EdgeRef;
        use petgraph::Direction::{Incoming, Outgoing};

        let Some(&idx) = self.by_name.get(&(NodeKind::Model, name.to_string())) else {
            return (Vec::new(), Vec::new());
        };
        let mut upstream: Vec<&NodeData> = self
            .graph
            .edges_directed(idx, Outgoing)
            .filter(|e| !matches!(e.weight(), EdgeKind::MacroCall))
            .map(|e| &self.graph[e.target()])
            .collect();
        let mut downstream: Vec<&NodeData> = self
            .graph
            .edges_directed(idx, Incoming)
            .filter(|e| matches!(e.weight(), EdgeKind::Ref))
            .map(|e| &self.graph[e.source()])
            .collect();
        upstream.sort_by(|a, b| a.name.cmp(&b.name));
        downstream.sort_by(|a, b| a.name.cmp(&b.name));
        (upstream, downstream)
    }

    /// Models calling a macro (incoming `MacroCall` edges).
    pub fn macro_call_sites<'a>(&'a self, name: &str) -> impl Iterator<Item = &'a NodeData> + 'a {
        self.neighbors(NodeKind::Macro, name, petgraph::Direction::Incoming)
    }

    fn neighbors<'a>(
        &'a self,
        kind: NodeKind,
        name: &str,
        direction: petgraph::Direction,
    ) -> impl Iterator<Item = &'a NodeData> + 'a {
        let target = self.by_name.get(&(kind, name.to_string())).copied();
        target.into_iter().flat_map(move |idx| {
            self.graph
                .neighbors_directed(idx, direction)
                .map(|n| &self.graph[n])
        })
    }

    /// The model/source DAG for `dbt.projectGraph`: every non-macro node
    /// with its layer (dependencies at 0, each dependent one past its
    /// deepest dependency) and the ref/source edges as
    /// (dependency name+kind, dependent name+kind) — left-to-right flow.
    #[allow(clippy::type_complexity)]
    pub fn dag(
        &self,
    ) -> (
        Vec<(&NodeData, usize)>,
        Vec<((NodeKind, String), (NodeKind, String))>,
    ) {
        use petgraph::visit::EdgeRef;
        use petgraph::Direction::Outgoing;

        fn depth(
            graph: &StableDiGraph<NodeData, EdgeKind>,
            idx: NodeIndex,
            memo: &mut HashMap<NodeIndex, usize>,
            visiting: &mut Vec<NodeIndex>,
        ) -> usize {
            if let Some(&d) = memo.get(&idx) {
                return d;
            }
            if visiting.contains(&idx) {
                return 0; // cycle guard: break at the current node
            }
            visiting.push(idx);
            let mut best = 0;
            for edge in graph.edges_directed(idx, Outgoing) {
                if matches!(edge.weight(), EdgeKind::MacroCall) {
                    continue;
                }
                best = best.max(1 + depth(graph, edge.target(), memo, visiting));
            }
            visiting.pop();
            memo.insert(idx, best);
            best
        }

        let depths = self.depths.get_or_init(|| {
            let mut memo = HashMap::new();
            for i in self.graph.node_indices() {
                if self.graph[i].kind != NodeKind::Macro {
                    depth(&self.graph, i, &mut memo, &mut Vec::new());
                }
            }
            memo
        });
        let mut nodes: Vec<(&NodeData, usize)> = self
            .graph
            .node_indices()
            .filter(|&i| self.graph[i].kind != NodeKind::Macro)
            .map(|i| (&self.graph[i], depths.get(&i).copied().unwrap_or(0)))
            .collect();
        nodes.sort_by(|a, b| (a.1, &a.0.name).cmp(&(b.1, &b.0.name)));

        let mut edges: Vec<((NodeKind, String), (NodeKind, String))> = self
            .graph
            .edge_indices()
            .filter_map(|e| {
                let (from, to) = self.graph.edge_endpoints(e)?;
                (!matches!(self.graph[e], EdgeKind::MacroCall)
                    && self.graph[from].kind != NodeKind::Macro
                    && self.graph[to].kind != NodeKind::Macro)
                    .then(|| {
                        // dependency first: the DAG flows dependency → dependent
                        (
                            (self.graph[to].kind, self.graph[to].name.clone()),
                            (self.graph[from].kind, self.graph[from].name.clone()),
                        )
                    })
            })
            .collect();
        edges.sort();
        edges.dedup();
        (nodes, edges)
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

/// Pure per-file edge extraction via the phase-1 pipeline (scan side).
fn extract_edge_seeds(text: &str, dialect: &Dialect) -> Vec<EdgeSeed> {
    let Ok(doc) = JinjaSqlParser.parse(text, dialect) else {
        return Vec::new();
    };
    doc.tokens
        .iter()
        .filter_map(|token| {
            let (kind, edge) = match token.kind {
                TokenKind::Ref => (NodeKind::Model, EdgeKind::Ref),
                TokenKind::SourceName => (NodeKind::Source, EdgeKind::Source),
                TokenKind::Macro => (NodeKind::Macro, EdgeKind::MacroCall),
                _ => return None,
            };
            Some((kind, edge, doc.literal(token).to_string()))
        })
        .collect()
}
