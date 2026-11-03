//! Transitive traversal (03 §10): BFS closures over the ref/source
//! edges (macro calls excluded), deduped, excluding the start node.
//! These power dead-model detection and dbt.impact.

use std::collections::HashSet;

use petgraph::visit::EdgeRef;
use petgraph::Direction::{Incoming, Outgoing};

use super::{EdgeKind, NodeData, NodeKind, ProjectGraph};

impl ProjectGraph {
    /// Everything that (transitively) depends on `name`: models,
    /// exposures, metrics — incoming non-macro edges, breadth-first.
    pub fn transitive_downstream<'a>(&'a self, name: &str) -> Vec<&'a NodeData> {
        self.closure(name, Incoming)
            .into_iter()
            .map(|(node, _)| node)
            .collect()
    }

    /// Everything `name` (transitively) depends on.
    pub fn transitive_upstream<'a>(&'a self, name: &str) -> Vec<&'a NodeData> {
        self.closure(name, Outgoing)
            .into_iter()
            .map(|(node, _)| node)
            .collect()
    }

    /// The downstream closure with BFS depth (1 = direct dependent),
    /// depth-then-name ordered — the dbt.impact payload shape.
    pub fn downstream_closure_with_depth<'a>(&'a self, name: &str) -> Vec<(&'a NodeData, usize)> {
        self.closure(name, Incoming)
    }

    fn closure<'a>(
        &'a self,
        name: &str,
        direction: petgraph::Direction,
    ) -> Vec<(&'a NodeData, usize)> {
        let Some(&start) = self.by_name.get(&(NodeKind::Model, name.to_string())) else {
            return Vec::new();
        };
        let mut seen: HashSet<_> = HashSet::from([start]);
        let mut frontier = vec![start];
        let mut out = Vec::new();
        let mut depth = 0usize;
        while !frontier.is_empty() {
            depth += 1;
            let mut next = Vec::new();
            for idx in frontier {
                for edge in self.graph.edges_directed(idx, direction) {
                    if matches!(edge.weight(), EdgeKind::MacroCall) {
                        continue;
                    }
                    let neighbor = match direction {
                        Incoming => edge.source(),
                        Outgoing => edge.target(),
                    };
                    if seen.insert(neighbor) {
                        out.push((&self.graph[neighbor], depth));
                        next.push(neighbor);
                    }
                }
            }
            frontier = next;
        }
        out.sort_by(|a, b| (a.1, &a.0.name).cmp(&(b.1, &b.0.name)));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::super::{EdgeKind, NodeKind, ProjectGraph};
    use std::path::PathBuf;

    /// a ← b ← d, a ← c ← d (diamond): closure dedups d.
    fn diamond() -> ProjectGraph {
        let mut graph = ProjectGraph::default();
        for name in ["a", "b", "c", "d"] {
            graph.ensure_node(NodeKind::Model, name.to_string(), PathBuf::from(name));
        }
        for (from, to) in [("b", "a"), ("c", "a"), ("d", "b"), ("d", "c")] {
            let f = graph.ensure_node(NodeKind::Model, from.to_string(), PathBuf::new());
            let t = graph.ensure_node(NodeKind::Model, to.to_string(), PathBuf::new());
            graph.graph.update_edge(f, t, EdgeKind::Ref);
        }
        graph
    }

    #[test]
    fn downstream_closure_dedups_and_orders_by_depth() {
        let graph = diamond();
        let closure = graph.downstream_closure_with_depth("a");
        let names: Vec<(&str, usize)> = closure
            .iter()
            .map(|(node, depth)| (node.name.as_str(), *depth))
            .collect();
        assert_eq!(names, vec![("b", 1), ("c", 1), ("d", 2)]);
        assert_eq!(graph.transitive_upstream("d").len(), 3);
        assert!(graph.transitive_downstream("d").is_empty());
        assert!(graph.transitive_downstream("nope").is_empty());
    }
}
