//! What the column graph answers. Every answer is sorted, so it does not
//! depend on the order the lineages were added in.

use std::collections::{HashSet, VecDeque};

use petgraph::stable_graph::NodeIndex;
use petgraph::visit::EdgeRef;
use petgraph::Direction;

use super::{ColumnGraph, ColumnNode, Flow, Relation, STAR};

/// A column a walk reached, and how many edges away from where it started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reached {
    pub node: NodeIndex,
    pub depth: usize,
}

impl ColumnGraph {
    /// The column `name` of `relation`, matched case-insensitively.
    pub fn find(&self, relation: &Relation, name: &str) -> Option<NodeIndex> {
        // boundary: the key owns its parts
        self.index
            .get(&(relation.clone(), name.to_lowercase()))
            .copied()
    }

    /// The model `model`'s column `name`.
    pub fn find_model_column(&self, model: &str, name: &str) -> Option<NodeIndex> {
        self.find(&Relation::Model(model.to_string()), name)
    }

    pub fn column(&self, node: NodeIndex) -> &ColumnNode {
        &self.graph[node]
    }

    /// The model's output columns: its lineage's order, then what its `*`
    /// rows resolved to. Empty for a model the graph does not hold.
    pub fn outputs(&self, model: &str) -> &[NodeIndex] {
        self.models.get(model).map_or(&[], Vec::as_slice)
    }

    /// The models the graph holds, by name.
    pub fn models(&self) -> impl Iterator<Item = &str> {
        self.models.keys().map(String::as_str)
    }

    /// The columns `node` reads directly, with how.
    pub fn reads(&self, node: NodeIndex) -> Vec<(NodeIndex, Flow)> {
        self.neighbours(node, Direction::Incoming)
    }

    /// The columns that read `node` directly, with how.
    pub fn read_by(&self, node: NodeIndex) -> Vec<(NodeIndex, Flow)> {
        self.neighbours(node, Direction::Outgoing)
    }

    /// Everything upstream of `start` (what it is computed from), up to
    /// `max_depth` edges away (`None`: all of it). `start` is not included.
    pub fn upstream(&self, start: &[NodeIndex], max_depth: Option<usize>) -> Vec<Reached> {
        self.walk(start, max_depth, Direction::Incoming)
    }

    /// Everything downstream of `start` (what reads it, at any remove), up
    /// to `max_depth` edges away (`None`: all of it). `start` is not
    /// included.
    pub fn downstream(&self, start: &[NodeIndex], max_depth: Option<usize>) -> Vec<Reached> {
        self.walk(start, max_depth, Direction::Outgoing)
    }

    /// Where `node` comes from under its own name: up while the column has
    /// exactly one upstream column, reached unchanged (`Direct` or `Star`)
    /// and named the same. The chain from `node` (first) to its origin
    /// (last), at most `max_hops` hops; a cycle ends it where it would
    /// repeat.
    pub fn origin_chain(&self, node: NodeIndex, max_hops: usize) -> Vec<NodeIndex> {
        let mut chain = vec![node];
        let mut seen = HashSet::from([node]);
        let mut at = node;
        for _ in 0..max_hops {
            let reads: Vec<(NodeIndex, Flow)> = self
                .reads(at)
                .into_iter()
                .filter(|(n, _)| self.graph[*n].column != STAR)
                .collect();
            let [(from, flow)] = reads.as_slice() else {
                break;
            };
            let same = self.graph[*from]
                .column
                .eq_ignore_ascii_case(&self.graph[at].column);
            if *flow == Flow::Derived || !same || !seen.insert(*from) {
                break;
            }
            chain.push(*from);
            at = *from;
        }
        chain
    }

    /// Breadth first from every start at once, each column at its least
    /// depth; sorted by depth, then relation, then column.
    fn walk(&self, start: &[NodeIndex], max_depth: Option<usize>, dir: Direction) -> Vec<Reached> {
        let mut seen: HashSet<NodeIndex> = start.iter().copied().collect();
        let mut queue: VecDeque<(NodeIndex, usize)> = start.iter().map(|&n| (n, 0)).collect();
        let mut out = Vec::new();
        while let Some((node, depth)) = queue.pop_front() {
            if max_depth.is_some_and(|max| depth >= max) {
                continue;
            }
            for next in self.graph.neighbors_directed(node, dir) {
                if seen.insert(next) {
                    out.push(Reached {
                        node: next,
                        depth: depth + 1,
                    });
                    queue.push_back((next, depth + 1));
                }
            }
        }
        out.sort_by(|a, b| {
            a.depth
                .cmp(&b.depth)
                .then_with(|| self.order(a.node).cmp(&self.order(b.node)))
        });
        out
    }

    fn neighbours(&self, node: NodeIndex, dir: Direction) -> Vec<(NodeIndex, Flow)> {
        let mut out: Vec<(NodeIndex, Flow)> = self
            .graph
            .edges_directed(node, dir)
            .map(|e| {
                let other = match dir {
                    Direction::Incoming => e.source(),
                    Direction::Outgoing => e.target(),
                };
                (other, *e.weight())
            })
            .collect();
        out.sort_by(|a, b| self.order(a.0).cmp(&self.order(b.0)));
        out
    }

    /// The sort key of a column: its relation, then its name ignoring case.
    fn order(&self, node: NodeIndex) -> (&Relation, String) {
        let column = &self.graph[node];
        (&column.relation, column.column.to_lowercase())
    }
}
