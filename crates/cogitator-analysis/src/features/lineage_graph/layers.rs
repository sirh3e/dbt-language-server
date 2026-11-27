//! Each node's layer, exactly as the Neovim plugin's `dag_data.relayer`
//! assigns it: the longest path from a root (Kahn's order), and nodes a
//! cycle keeps from the order one layer past the deepest.

use std::collections::HashMap;

use super::{LineageEdge, LineageNode};

pub(super) fn assign(nodes: &mut [LineageNode], edges: &[LineageEdge]) {
    let index: HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();
    let mut indegree = vec![0usize; nodes.len()];
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for edge in edges {
        let (Some(&from), Some(&to)) = (index.get(edge.from.as_str()), index.get(edge.to.as_str()))
        else {
            continue;
        };
        if from != to {
            indegree[to] += 1;
            children[from].push(to);
        }
    }
    let mut layer = vec![0usize; nodes.len()];
    let mut queue: Vec<usize> = (0..nodes.len()).filter(|&i| indegree[i] == 0).collect();
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
    for (i, node) in nodes.iter_mut().enumerate() {
        node.layer = if indegree[i] == 0 {
            layer[i]
        } else {
            deepest + 1
        };
    }
}
