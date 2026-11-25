//! Columns known only from downstream: a hop into a node that does not
//! list the column adds it there, and every hop is spelled as its target.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::{SiteColumn, SiteNode};

/// A column a hop names but its node does not list is added to that node
/// (known only from downstream), and every hop is spelled as its target.
pub(super) fn add_referenced(nodes: &mut [SiteNode]) {
    let index: HashMap<String, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.clone(), i)) // boundary: a lookup-only index
        .collect();
    let mut wanted: BTreeMap<usize, BTreeSet<String>> = BTreeMap::new();
    for node in nodes.iter() {
        for hop in node.columns.iter().flat_map(|c| &c.inputs) {
            if let Some(&target) = hop.node.as_ref().and_then(|id| index.get(id)) {
                wanted.entry(target).or_default().insert(hop.column.clone());
            }
        }
    }
    for (target, names) in wanted {
        let node = &mut nodes[target];
        for name in names {
            if node
                .columns
                .iter()
                .any(|c| c.name.eq_ignore_ascii_case(&name))
            {
                continue;
            }
            node.columns.push(SiteColumn {
                name,
                description: String::new(),
                data_type: None,
                documented: false,
                produced: false,
                tests: Vec::new(),
                inputs: Vec::new(),
                star: false,
                transform: None,
                detail: None,
            });
        }
    }
    let spelled: HashMap<(usize, String), String> = nodes
        .iter()
        .enumerate()
        .flat_map(|(i, n)| {
            n.columns
                .iter()
                .map(move |c| ((i, c.name.to_ascii_lowercase()), c.name.clone()))
        })
        .collect();
    for node in nodes.iter_mut() {
        for hop in node.columns.iter_mut().flat_map(|c| c.inputs.iter_mut()) {
            let Some(&target) = hop.node.as_ref().and_then(|id| index.get(id)) else {
                continue;
            };
            if let Some(name) = spelled.get(&(target, hop.column.to_ascii_lowercase())) {
                hop.column.clone_from(name);
            }
        }
    }
}
