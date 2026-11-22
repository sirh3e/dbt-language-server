//! `dbt.syntaxTree` end to end: the tree-sitter-dbt tree of the open
//! model, as the Neovim plugin's :DbtSyntaxTree draws it.

mod harness;
use harness::jaffle;
use serde_json::json;

#[test]
fn syntax_tree_lists_the_open_models_nodes() {
    let project = jaffle();
    let (mut lsp, uri) = project.booted();
    let response = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.syntaxTree", "arguments": [{"uri": uri}]}),
    );
    let result = &response["result"];
    let nodes = result["nodes"].as_array().expect("nodes");
    assert_eq!(result["truncated"], false, "{response}");
    assert_eq!(nodes[0]["depth"], 0, "{response}");
    assert_eq!(
        nodes[0]["range"]["start"],
        json!({"line": 0, "character": 0})
    );
    assert!(
        nodes.iter().any(|n| n["kind"] == "output"),
        "customers.sql's {{{{ ref() }}}} calls are output nodes: {response}"
    );
}

#[test]
fn syntax_tree_needs_an_open_document() {
    let project = jaffle();
    let (mut lsp, _uri) = project.booted();
    let response = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.syntaxTree", "arguments": [{"uri": "file:///nowhere.sql"}]}),
    );
    assert!(
        response["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("open document")),
        "{response}"
    );
}
