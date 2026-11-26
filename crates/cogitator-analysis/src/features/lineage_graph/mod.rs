//! `dbt.columnLineageGraph`: the column lineage around one model column,
//! walked across the project in one answer. Pure: the walk asks a lookup
//! for each model's [`ModelLineage`] and never reads a file.
//!
//! The payload is the one the Neovim plugin's `column_lineage.walk`
//! assembled from one `dbt.columnLineage` request per hop: the same node
//! ids, names and fields, nodes in the order the walk first meets them,
//! edges once each, and each node's `layer` as `dag_data.relayer` gives
//! it. Beyond that walk an edge may say `kind` (`direct` when the column
//! passes its input through unchanged, else `derived`) and a model column
//! its `compiledUri`.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use serde::Serialize;

use super::{GraphNodeKind, ProjectGraph};
use crate::lineage::{ModelLineage, SourceRef};

mod layers;
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
    /// Models the lookup has no answer for yet (still being computed).
    pub stale: Vec<String>,
    /// Every model the walk asked about, in order.
    #[serde(skip)]
    pub asked: Vec<String>,
}

/// What a lookup knows of one model.
#[derive(Debug, Clone)]
pub enum Known {
    /// Computed: `None` inside is a model whose lineage declined.
    Lineage(Arc<Option<ModelLineage>>),
    /// Not computed yet.
    Pending,
}

/// Where the walk starts and how far it goes.
pub struct WalkInput<'a> {
    pub project: &'a ProjectGraph,
    pub model: &'a str,
    pub column: Option<&'a str>,
    pub direction: Direction,
    /// Hops each way, at least 1.
    pub depth: usize,
    /// A model's compiled file, as a uri, when one is on disk.
    pub compiled_uri: &'a dyn Fn(&str) -> Option<String>,
}

#[derive(Debug, Clone)]
struct Task {
    dir: Dir,
    model: String,
    column: Option<String>,
    depth: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Dir {
    Start,
    Up,
    Down,
}

struct Walk<'a> {
    input: &'a WalkInput<'a>,
    owners: HashMap<&'a str, (GraphNodeKind, Option<&'a str>)>,
    children: HashMap<&'a str, Vec<&'a str>>,
    nodes: Vec<LineageNode>,
    seen_nodes: HashSet<String>,
    edges: Vec<LineageEdge>,
    seen_edges: HashSet<(String, String)>,
    known: HashMap<String, Option<Arc<Option<ModelLineage>>>>,
    tasks: VecDeque<Task>,
    queued: HashSet<(Dir, String, Option<String>)>,
    result: WalkResult,
}

/// Walks the lineage around `input`'s start, asking `lookup` once per model.
pub fn walk(input: &WalkInput<'_>, lookup: &dyn Fn(&str) -> Known) -> WalkResult {
    let mut w = Walk::new(input);
    w.push(Task {
        dir: Dir::Start,
        model: input.model.to_string(),
        column: input.column.map(str::to_string),
        depth: input.depth.max(1),
    });
    loop {
        while let Some(task) = w.tasks.pop_front() {
            match task.dir {
                Dir::Down => w.downstream(&task, lookup),
                Dir::Start | Dir::Up => {
                    let lineage = w.lineage_of(&task.model, lookup);
                    w.upstream(&task, lineage.as_deref().and_then(Option::as_ref));
                }
            }
        }
        // a column the lineage does not list (select *, a declined
        // template) still gets its node and its consumers
        match input.column {
            Some(column) if w.result.start.is_none() => {
                let id = w.model_column(input.model, column);
                w.result.start = Some(id);
                if input.direction.down() {
                    w.push(Task {
                        dir: Dir::Down,
                        model: input.model.to_string(),
                        column: Some(column.to_string()),
                        depth: input.depth.max(1),
                    });
                }
            }
            _ => break,
        }
    }
    w.finish()
}

impl<'a> Walk<'a> {
    fn new(input: &'a WalkInput<'a>) -> Self {
        let owners = input
            .project
            .nodes
            .iter()
            .map(|n| (n.id.as_str(), (n.kind, n.uri.as_deref())))
            .collect();
        let mut children: HashMap<&str, Vec<&str>> = HashMap::new();
        for edge in &input.project.edges {
            children
                .entry(edge.from.as_str())
                .or_default()
                .push(edge.to.as_str());
        }
        Walk {
            input,
            owners,
            children,
            nodes: Vec::new(),
            seen_nodes: HashSet::new(),
            edges: Vec::new(),
            seen_edges: HashSet::new(),
            known: HashMap::new(),
            tasks: VecDeque::new(),
            queued: HashSet::new(),
            result: WalkResult::default(),
        }
    }

    fn push(&mut self, task: Task) {
        let key = (task.dir, task.model.clone(), task.column.clone());
        if self.queued.insert(key) {
            self.tasks.push_back(task);
        }
    }

    /// A model's lineage, asked once; `None` for no file, pending or declined.
    fn lineage_of(
        &mut self,
        model: &str,
        lookup: &dyn Fn(&str) -> Known,
    ) -> Option<Arc<Option<ModelLineage>>> {
        if let Some(known) = self.known.get(model) {
            return known.clone();
        }
        let has_file = matches!(
            self.owners.get(format!("model:{model}").as_str()),
            Some((_, Some(uri))) if !uri.is_empty()
        );
        let found = if has_file {
            self.result.asked.push(model.to_string());
            match lookup(model) {
                Known::Lineage(lineage) => {
                    if lineage.is_none() {
                        self.result.missing.push(model.to_string());
                    }
                    Some(lineage)
                }
                Known::Pending => {
                    self.result.stale.push(model.to_string());
                    None
                }
            }
        } else {
            None
        };
        self.known.insert(model.to_string(), found.clone());
        found
    }

    fn upstream(&mut self, task: &Task, lineage: Option<&ModelLineage>) {
        let direction = self.input.direction;
        for entry in lineage.map_or(&[][..], |l| &l.columns[..]) {
            if task.column.as_deref().is_some_and(|c| c != entry.name) {
                continue;
            }
            let here = self.model_column(&task.model, &entry.name);
            if task.dir == Dir::Start {
                self.result.start.get_or_insert_with(|| here.clone());
                if direction.down() {
                    self.push(Task {
                        dir: Dir::Down,
                        model: task.model.clone(),
                        column: Some(entry.name.clone()),
                        depth: self.input.depth.max(1),
                    });
                }
                if !direction.up() {
                    continue;
                }
            }
            let kind = if entry.direct { "direct" } else { "derived" };
            for source in &entry.sources {
                let from = self.ref_node(source);
                self.edge(from, here.clone(), kind);
                if let SourceRef::Model { name, column } = source {
                    if task.depth > 1 {
                        self.push(Task {
                            dir: Dir::Up,
                            model: name.clone(),
                            column: Some(column.clone()),
                            depth: task.depth - 1,
                        });
                    }
                }
            }
        }
    }

    fn downstream(&mut self, task: &Task, lookup: &dyn Fn(&str) -> Known) {
        let column = task.column.clone().unwrap_or_default();
        let from = self.model_column(&task.model, &column);
        let kids: Vec<String> = self
            .children
            .get(format!("model:{}", task.model).as_str())
            .into_iter()
            .flatten()
            .filter_map(|id| match self.owners.get(id) {
                Some((GraphNodeKind::Model, _)) => id.strip_prefix("model:").map(str::to_string),
                _ => None,
            })
            .collect();
        for child in kids {
            let lineage = self.lineage_of(&child, lookup);
            let Some(lineage) = lineage.as_deref().and_then(Option::as_ref) else {
                continue;
            };
            for entry in &lineage.columns {
                let reads = entry.sources.iter().any(|s| {
                    matches!(s, SourceRef::Model { name, column: c }
                        if *name == task.model && *c == column)
                });
                if !reads {
                    continue;
                }
                let to = self.model_column(&child, &entry.name);
                let kind = if entry.direct { "direct" } else { "derived" };
                self.edge(from.clone(), to, kind);
                if task.depth > 1 {
                    self.push(Task {
                        dir: Dir::Down,
                        model: child.clone(),
                        column: Some(entry.name.clone()),
                        depth: task.depth - 1,
                    });
                }
            }
        }
    }

    fn uri_of(&self, id: &str) -> Option<String> {
        self.owners
            .get(id)
            .and_then(|(_, uri)| uri.map(str::to_string))
    }

    fn model_column(&mut self, model: &str, column: &str) -> String {
        let id = format!("model:{model}::{column}");
        if !self.seen_nodes.contains(&id) {
            let node = LineageNode {
                id: id.clone(),
                name: format!("{model}.{column}"),
                kind: "model",
                uri: self.uri_of(&format!("model:{model}")),
                tags: Vec::new(),
                columns: Vec::new(),
                model: Some(model.to_string()),
                column: column.to_string(),
                layer: 0,
                compiled_uri: (self.input.compiled_uri)(model),
            };
            self.add(node);
        }
        id
    }

    fn ref_node(&mut self, source: &SourceRef) -> String {
        let node = match source {
            SourceRef::Model { name, column } => return self.model_column(name, column),
            SourceRef::Source {
                source,
                table,
                column,
            } => LineageNode {
                id: format!("source:{source}.{table}::{column}"),
                name: format!("{source}.{table}.{column}"),
                kind: "source",
                uri: self.uri_of(&format!("source:{source}")),
                tags: Vec::new(),
                columns: Vec::new(),
                model: None,
                column: column.clone(),
                layer: 0,
                compiled_uri: None,
            },
            SourceRef::Unknown { name, column } => LineageNode {
                id: format!("unknown:{name}::{column}"),
                name: format!("{name}.{column}"),
                kind: "unknown",
                uri: None,
                tags: Vec::new(),
                columns: Vec::new(),
                model: None,
                column: column.clone(),
                layer: 0,
                compiled_uri: None,
            },
        };
        let id = node.id.clone();
        if !self.seen_nodes.contains(&id) {
            self.add(node);
        }
        id
    }

    fn add(&mut self, node: LineageNode) {
        self.seen_nodes.insert(node.id.clone());
        self.nodes.push(node);
    }

    fn edge(&mut self, from: String, to: String, kind: &'static str) {
        if self.seen_edges.insert((from.clone(), to.clone())) {
            self.edges.push(LineageEdge { from, to, kind });
        }
    }

    fn finish(mut self) -> WalkResult {
        layers::assign(&mut self.nodes, &self.edges);
        WalkResult {
            nodes: self.nodes,
            edges: self.edges,
            ..self.result
        }
    }
}
