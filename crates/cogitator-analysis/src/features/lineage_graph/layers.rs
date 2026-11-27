//! Each node's layer, exactly as the Neovim plugin's `dag_data.relayer`
//! assigns it: the longest path from a root (Kahn's order), and nodes a
//! cycle keeps from the order one layer past the deepest.

use std::collections::HashMap;

use super::{LineageEdge, LineageNode};

pub(super) fn assign(nodes: &mut [LineageNode], edges: &[LineageEdge]) {
    let ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    let pairs: Vec<(&str, &str)> = edges
        .iter()
        .map(|e| (e.from.as_str(), e.to.as_str()))
        .collect();
    let layers = longest_path(&ids, &pairs);
    for (node, layer) in nodes.iter_mut().zip(layers) {
        node.layer = layer;
    }
}

/// The layer of each of `ids`, in their order, over `edges` (parent →
/// child; an edge naming an unknown id, and a self-loop, are ignored).
pub(crate) fn longest_path(ids: &[&str], edges: &[(&str, &str)]) -> Vec<usize> {
    let index: HashMap<&str, usize> = ids.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    let mut indegree = vec![0usize; ids.len()];
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); ids.len()];
    for (from, to) in edges {
        let (Some(&from), Some(&to)) = (index.get(from), index.get(to)) else {
            continue;
        };
        if from != to {
            indegree[to] += 1;
            children[from].push(to);
        }
    }
    let mut layer = vec![0usize; ids.len()];
    let mut queue: Vec<usize> = (0..ids.len()).filter(|&i| indegree[i] == 0).collect();
    let mut head = 0;
    let mut deepest = 0;
    while head < queue.len() {
        let id = queue[head];
        head += 1;
        for &child in &children[id] {
            layer[child] = layer[child].max(layer[id] + 1);
            deepest = deepest.max(layer[child]);
            indegree[child] -= 1;
            if indegree[child] == 0 {
                queue.push(child);
            }
        }
    }
    (0..ids.len())
        .map(|i| {
            if indegree[i] == 0 {
                layer[i]
            } else {
                deepest + 1
            }
        })
        .collect()
}
