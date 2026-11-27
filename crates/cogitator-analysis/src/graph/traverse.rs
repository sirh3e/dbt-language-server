//! Transitive traversal (03 §10): BFS closures over the ref/source
//! edges (macro calls excluded), deduped, excluding the start node.
//! These power dead-model detection and dbt.impact.

use std::collections::{HashMap, HashSet};

use petgraph::stable_graph::NodeIndex;

use petgraph::visit::EdgeRef;
use petgraph::Direction::{Incoming, Outgoing};

use super::{EdgeKind, NodeData, NodeKind, ProjectGraph};

impl ProjectGraph {
    /// The model `model` and the models within `depth` hops of it,
    /// upstream (`up`: what it refs, models only) and downstream (`down`:
    /// what refs it), each once, in the order first found: the start, then
    /// the upstream walk's, then the downstream walk's, breadth first, each
    /// step's neighbours by name.
    pub fn models_within(&self, model: &str, up: bool, down: bool, depth: usize) -> Vec<String> {
        let mut found = vec![model.to_string()];
        let mut seen: HashSet<String> = found.iter().cloned().collect();
        for (go, upward) in [(up, true), (down, false)] {
            if !go {
                continue;
            }
            let mut queue = std::collections::VecDeque::from([(model.to_string(), 0usize)]);
            let mut level: HashMap<String, usize> = HashMap::new();
            while let Some((name, hops)) = queue.pop_front() {
                if hops >= depth {
                    continue;
                }
                let mut next: Vec<String> = if upward {
                    self.upstream_of(&name)
                        .filter(|n| n.kind == NodeKind::Model)
                        .map(|n| n.name.clone())
                        .collect()
                } else {
                    self.downstream_of(&name).map(|n| n.name.clone()).collect()
                };
                next.sort();
                for n in next {
                    if level.insert(n.clone(), hops + 1).is_none() {
                        if seen.insert(n.clone()) {
                            found.push(n.clone());
                        }
                        queue.push_back((n, hops + 1));
                    }
                }
            }
        }
        found
    }

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

    /// The longest `weight`-weighted chain of models through ref edges —
    /// the critical path of a build: every model on it waits for the one
    /// before. Upstream first; `(path, total weight)`. A cycle (which dbt
    /// refuses anyway) answers an empty path rather than loop.
    pub fn critical_path(&self, weight: &dyn Fn(&str) -> f64) -> (Vec<String>, f64) {
        let models: Vec<NodeIndex> = self
            .graph
            .node_indices()
            .filter(|&idx| self.graph[idx].kind == NodeKind::Model)
            .collect();
        let mut best: HashMap<NodeIndex, Option<(f64, Option<NodeIndex>)>> = HashMap::new();
        for &start in &models {
            if self
                .longest_to(start, weight, &mut best, &mut HashSet::new())
                .is_none()
            {
                return (Vec::new(), 0.0);
            }
        }
        let Some((&end, &(total, _))) = best
            .iter()
            .filter_map(|(idx, value)| value.as_ref().map(|v| (idx, v)))
            .max_by(|a, b| {
                a.1 .0
                    .total_cmp(&b.1 .0)
                    .then_with(|| self.graph[*b.0].name.cmp(&self.graph[*a.0].name))
            })
        else {
            return (Vec::new(), 0.0);
        };
        let mut path = Vec::new();
        let mut at = Some(end);
        while let Some(idx) = at {
            path.push(self.graph[idx].name.clone());
            at = best.get(&idx).and_then(|v| v.as_ref()).and_then(|v| v.1);
        }
        path.reverse();
        (path, total)
    }

    /// The heaviest chain ending at `idx` (its own weight included), and
    /// the upstream model it comes through; `None` on a cycle.
    fn longest_to(
        &self,
        idx: NodeIndex,
        weight: &dyn Fn(&str) -> f64,
        best: &mut HashMap<NodeIndex, Option<(f64, Option<NodeIndex>)>>,
        on_stack: &mut HashSet<NodeIndex>,
    ) -> Option<f64> {
        if let Some(done) = best.get(&idx) {
            return done.map(|(total, _)| total);
        }
        if !on_stack.insert(idx) {
            return None;
        }
        let mut upstream: Vec<NodeIndex> = self
            .graph
            .edges_directed(idx, Outgoing)
            .filter(|e| matches!(e.weight(), EdgeKind::Ref))
            .map(|e| e.target())
            .filter(|&t| self.graph[t].kind == NodeKind::Model)
            .collect();
        upstream.sort_by(|a, b| self.graph[*a].name.cmp(&self.graph[*b].name));
        let mut via: Option<(f64, NodeIndex)> = None;
        for up in upstream {
            let total = self.longest_to(up, weight, best, on_stack)?;
            if via.is_none_or(|(t, _)| total > t) {
                via = Some((total, up));
            }
        }
        on_stack.remove(&idx);
        let own = weight(&self.graph[idx].name).max(0.0);
        let total = own + via.map(|(t, _)| t).unwrap_or(0.0);
        best.insert(idx, Some((total, via.map(|(_, up)| up))));
        Some(total)
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
    fn models_within_a_depth_each_way() {
        let graph = diamond();
        // d refs b and c, which ref a: upstream of d first, then nothing down
        assert_eq!(graph.models_within("d", true, false, 1), ["d", "b", "c"]);
        assert_eq!(
            graph.models_within("d", true, false, 2),
            ["d", "b", "c", "a"]
        );
        // both ways from b: a upstream, d downstream, each once
        assert_eq!(graph.models_within("b", true, true, 3), ["b", "a", "d"]);
        assert_eq!(graph.models_within("a", false, false, 3), ["a"]);
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

    #[test]
    fn critical_path_takes_the_heaviest_chain() {
        let graph = diamond();
        let seconds = |name: &str| match name {
            "a" => 1.0,
            "b" => 2.0,
            "c" => 5.0,
            "d" => 1.0,
            _ => 0.0,
        };
        let (path, total) = graph.critical_path(&seconds);
        assert_eq!(path, ["a", "c", "d"]);
        assert_eq!(total, 7.0);
        assert_eq!(ProjectGraph::default().critical_path(&seconds).0.len(), 0);
    }

    #[test]
    fn critical_path_refuses_a_cycle() {
        let mut graph = diamond();
        let a = graph.ensure_node(NodeKind::Model, "a".to_string(), PathBuf::new());
        let d = graph.ensure_node(NodeKind::Model, "d".to_string(), PathBuf::new());
        graph.graph.update_edge(a, d, EdgeKind::Ref);
        assert!(graph.critical_path(&|_| 1.0).0.is_empty());
    }
}
