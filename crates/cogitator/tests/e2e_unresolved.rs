//! `dbt.unresolved` end to end: every unresolved name of the project in
//! one list, unsaved edits of open documents included.

mod harness;
use harness::jaffle;
use serde_json::json;

#[test]
fn the_project_lists_what_does_not_resolve_unsaved_edits_included() {
    let project = jaffle();
    let (mut lsp, uri) = project.booted();
    let clean = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.unresolved", "arguments": [{}]}),
    );
    assert_eq!(clean["result"]["count"], 0, "{clean}");

    lsp.notify(
        "textDocument/didChange",
        json!({
            "textDocument": {"uri": uri, "version": 2},
            "contentChanges": [{"text": "select {{ ghost_macro() }}\nfrom {{ ref('ghost') }}\n"}]
        }),
    );
    let response = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.unresolved", "arguments": [{}]}),
    );
    let result = &response["result"];
    assert_eq!(result["count"], 2, "{response}");
    assert_eq!(
        result["byKind"],
        json!({"macro": 1, "ref": 1}),
        "{response}"
    );
    let items = result["items"].as_array().unwrap();
    assert_eq!(items[0]["kind"], "macro", "{response}");
    assert_eq!(items[0]["name"], "ghost_macro", "{response}");
    assert_eq!(items[1]["kind"], "ref", "{response}");
    assert_eq!(items[1]["name"], "ghost", "{response}");
    assert_eq!(items[1]["uri"], uri, "{response}");
    assert_eq!(items[1]["range"]["start"]["line"], 1, "{response}");

    let only = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.unresolved", "arguments": [{"kinds": ["ref"]}]}),
    );
    assert_eq!(only["result"]["count"], 1, "{only}");
    assert_eq!(only["result"]["items"][0]["name"], "ghost", "{only}");
}
