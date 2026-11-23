//! `dbt.select`: the nodes a selector string picks, as `dbt ls` lists
//! them (eager indirect selection), with where each is defined.

use cogitator_lsp::Range;
use serde::Serialize;

use crate::selectors::{self, Definition, Indirect, Node, Universe};

#[derive(Debug, Serialize, PartialEq)]
pub struct SelectedNode {
    pub uri: String,
    pub name: String,
    pub kind: &'static str,
    /// dbt's unique id, without a test's hash suffix.
    #[serde(rename = "uniqueId")]
    pub unique_id: String,
    pub range: Range,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct SelectAnswer {
    pub selector: String,
    pub count: usize,
    pub nodes: Vec<SelectedNode>,
    /// What the selector asks that is not evaluated (unknown methods or
    /// selectors, cycles, state methods), in words.
    pub issues: Vec<String>,
}

/// Evaluate `selector`; a string that does not parse is an error in words.
pub fn select_command(
    u: &Universe,
    defs: &[Definition],
    selector: &str,
) -> Result<SelectAnswer, String> {
    let eval = selectors::select(u, defs, selector, Indirect::Eager)
        .map_err(|err| format!("invalid selector: {}", err.message))?;
    let nodes: Vec<SelectedNode> = selectors::sorted(u, &eval)
        .into_iter()
        .map(selected)
        .collect();
    Ok(SelectAnswer {
        selector: selector.to_string(),
        count: nodes.len(),
        nodes,
        issues: eval.issues.iter().map(selectors::issue_message).collect(),
    })
}

fn selected(n: &Node) -> SelectedNode {
    let id_tail = match (&n.source, &n.version_number) {
        (Some((s, t)), _) => format!("{s}.{t}"),
        (None, Some(v)) => format!("{}.v{v}", n.name),
        (None, None) => n.name.clone(),
    };
    SelectedNode {
        uri: if n.uri.as_os_str().is_empty() {
            String::new()
        } else {
            cogitator_lsp::fs_path_to_uri(&n.uri)
        },
        name: n.display_name(),
        kind: n.resource_type.as_str(),
        unique_id: format!("{}.{}.{id_tail}", n.resource_type.as_str(), n.package),
        range: n.range,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selectors::ResourceType;

    #[test]
    fn answers_list_nodes_and_issues() {
        let mut u = Universe::default();
        let mut m = Node::new(ResourceType::Model, "orders", "p");
        m.uri = std::path::PathBuf::from("/p/models/orders.sql");
        u.push(m);
        let answer = select_command(&u, &[], "orders state:modified").unwrap();
        assert_eq!(answer.count, 1);
        assert_eq!(answer.nodes[0].unique_id, "model.p.orders");
        assert!(answer.nodes[0].uri.ends_with("/p/models/orders.sql"));
        assert_eq!(answer.issues.len(), 1);
        assert!(select_command(&u, &[], "@+x")
            .unwrap_err()
            .contains("cannot be combined"));
    }
}
