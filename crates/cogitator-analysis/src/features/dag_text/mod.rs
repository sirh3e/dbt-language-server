//! `dbt.dagText`: the project DAG, or a model's neighbourhood, drawn as
//! text — the picture the Neovim plugin's `:DbtDag` draws client-side
//! (`editors/nvim/lua/cogitator/{dag_data,layout,canvas}.lua`), for a
//! client that can only open a document (Zed). Pure: a [`Dag`] in, a
//! string out; the layout rules and the characters are the Lua's, so the
//! two draw the same picture of the same graph.
//!
//! Layers come from the server for the whole project (as `:DbtDag` takes
//! them from `dbt.projectGraph`) and are assigned again for a focus, as
//! `dag_data.focus` does. A project larger than [`MAX_NODES`] is cut to
//! its first layers, and the text says so.

use std::collections::HashSet;
use std::fmt::Write as _;

use super::graph_nav::ProjectGraph;
use super::lineage_graph::layers::longest_path;

mod canvas;
mod layout;
#[cfg(test)]
mod tests;

pub use canvas::{draw, GUTTER};
pub use layout::{layout, node_lines, Placed, MAX_COLUMNS};

/// The most nodes a drawing holds; the rest are left out and said.
pub const MAX_NODES: usize = 300;

/// One node of the drawing.
#[derive(Debug, Clone, PartialEq)]
pub struct DagNode {
    pub id: String,
    pub name: String,
    /// The wire word of its kind (`model`, `source`, `semantic_model`, …).
    pub kind: String,
    pub layer: usize,
    pub tags: Vec<String>,
    /// Documented column names, shown when the node is expanded.
    pub columns: Vec<String>,
}

/// A dependency → dependent edge.
#[derive(Debug, Clone, PartialEq)]
pub struct DagEdge {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Dag {
    pub nodes: Vec<DagNode>,
    pub edges: Vec<DagEdge>,
}

impl Dag {
    /// The `dbt.projectGraph` payload as a drawing's input.
    pub fn of(graph: &ProjectGraph) -> Self {
        Dag {
            nodes: graph
                .nodes
                .iter()
                .map(|n| DagNode {
                    id: n.id.clone(),
                    name: n.name.clone(),
                    kind: n.kind.as_str().to_string(),
                    layer: n.layer,
                    tags: n.tags.clone(),
                    columns: n.columns.iter().map(|c| c.name.clone()).collect(),
                })
                .collect(),
            edges: graph
                .edges
                .iter()
                .map(|e| DagEdge {
                    from: e.from.clone(),
                    to: e.to.clone(),
                })
                .collect(),
        }
    }

    fn has(&self, id: &str) -> bool {
        self.nodes.iter().any(|n| n.id == id)
    }
}

/// The same nodes with `layer` set as `dag_data.relayer` sets it.
pub fn relayer(nodes: Vec<DagNode>, edges: &[DagEdge]) -> Vec<DagNode> {
    let ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    let pairs: Vec<(&str, &str)> = edges
        .iter()
        .map(|e| (e.from.as_str(), e.to.as_str()))
        .collect();
    let layers = longest_path(&ids, &pairs);
    nodes
        .into_iter()
        .zip(layers)
        .map(|(node, layer)| DagNode { layer, ..node })
        .collect()
}

/// The part of `dag` within `depth` hops (at least 1) of `id`, upstream
/// and downstream but not sideways, with fresh layers; `dag_data.focus`.
/// An unknown id gives the graph back unchanged.
pub fn focus(dag: &Dag, id: &str, depth: usize) -> Dag {
    if !dag.has(id) {
        return dag.clone();
    }
    let mut keep: HashSet<&str> = HashSet::from([id]);
    for upward in [true, false] {
        let mut frontier = vec![id];
        for _ in 0..depth.max(1) {
            let mut next = Vec::new();
            for at in frontier {
                for edge in &dag.edges {
                    let (here, other) = if upward {
                        (&edge.to, &edge.from)
                    } else {
                        (&edge.from, &edge.to)
                    };
                    if here == at && keep.insert(other.as_str()) {
                        next.push(other.as_str());
                    }
                }
            }
            frontier = next;
        }
    }
    let nodes: Vec<DagNode> = dag
        .nodes
        .iter()
        .filter(|n| keep.contains(n.id.as_str()))
        .cloned()
        .collect();
    let edges: Vec<DagEdge> = dag
        .edges
        .iter()
        .filter(|e| keep.contains(e.from.as_str()) && keep.contains(e.to.as_str()))
        .cloned()
        .collect();
    Dag {
        nodes: relayer(nodes, &edges),
        edges,
    }
}

/// At most `max` nodes: the first layers, by name within a layer, the
/// graph's own order kept; edges between what is left.
pub fn bounded(dag: &Dag, max: usize) -> (Dag, bool) {
    if dag.nodes.len() <= max {
        return (dag.clone(), false);
    }
    let mut order: Vec<&DagNode> = dag.nodes.iter().collect();
    order.sort_by(|a, b| (a.layer, &a.name, &a.id).cmp(&(b.layer, &b.name, &b.id)));
    let kept: HashSet<&str> = order.iter().take(max).map(|n| n.id.as_str()).collect();
    let cut = Dag {
        nodes: dag
            .nodes
            .iter()
            .filter(|n| kept.contains(n.id.as_str()))
            .cloned()
            .collect(),
        edges: dag
            .edges
            .iter()
            .filter(|e| kept.contains(e.from.as_str()) && kept.contains(e.to.as_str()))
            .cloned()
            .collect(),
    };
    (cut, true)
}

/// Which nodes show their documented columns.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum Expanded {
    /// None, as `:DbtDag` opens.
    #[default]
    None,
    All,
    Some(HashSet<String>),
}

impl Expanded {
    fn holds(&self, id: &str) -> bool {
        match self {
            Expanded::None => false,
            Expanded::All => true,
            Expanded::Some(ids) => ids.contains(id),
        }
    }
}

/// The drawing's lines, exactly as `layout.layout` + `canvas.draw` give
/// them to the Neovim buffer (right-trimmed, a blank first row and column).
pub fn picture(dag: &Dag, expanded: &Expanded) -> Vec<String> {
    let (boxes, width, height) = layout(dag, &|id| expanded.holds(id));
    draw(&boxes, &dag.edges, width + 2, height + 1)
}

/// What is drawn, for the heading.
#[derive(Debug, Clone, PartialEq)]
pub enum Subject<'a> {
    Project,
    Around { model: &'a str, depth: usize },
}

/// The `dbt.dagText` answer.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct DagText {
    pub text: String,
    /// How many nodes the drawing holds.
    pub nodes: usize,
    /// Whether nodes were left out ([`MAX_NODES`]).
    pub truncated: bool,
}

/// The heading, the drawing, and what was left out, as one text.
pub fn render(dag: &Dag, subject: &Subject<'_>, expanded: &Expanded, max: usize) -> DagText {
    let total = dag.nodes.len();
    let (dag, truncated) = bounded(dag, max);
    let mut text = String::new();
    let what = match subject {
        Subject::Project => "the project".to_string(),
        Subject::Around { model, depth } => {
            let hops = if *depth == 1 { "hop" } else { "hops" };
            format!("{model}, {depth} {hops} up and down")
        }
    };
    let _ = writeln!(
        text,
        "dbt DAG — {what}: {} {}, {} {}",
        dag.nodes.len(),
        plural(dag.nodes.len(), "node", "nodes"),
        dag.edges.len(),
        plural(dag.edges.len(), "edge", "edges"),
    );
    text.push_str("Read left to right: each ▸ points from a node to what depends on it.\n");
    if truncated {
        let _ = writeln!(
            text,
            "Showing the first {} of {total} nodes (the first layers); draw a model's neighbourhood for the rest.",
            dag.nodes.len()
        );
    }
    text.push('\n');
    if dag.nodes.is_empty() {
        text.push_str("(no nodes)\n");
    } else {
        let mut lines = picture(&dag, expanded);
        while lines.last().is_some_and(|l| l.is_empty()) {
            lines.pop();
        }
        for line in lines {
            text.push_str(&line);
            text.push('\n');
        }
    }
    DagText {
        text,
        nodes: dag.nodes.len(),
        truncated,
    }
}

fn plural<'a>(n: usize, one: &'a str, many: &'a str) -> &'a str {
    if n == 1 {
        one
    } else {
        many
    }
}

/// The file name the text is written under (no directory).
pub fn file_name(model: Option<&str>) -> String {
    let safe = |s: &str| -> String {
        s.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    };
    match model {
        Some(model) => format!("{}.dag.txt", safe(model)),
        None => "project.dag.txt".to_string(),
    }
}
