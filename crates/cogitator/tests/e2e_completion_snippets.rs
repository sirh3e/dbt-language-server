//! Completion fitted to the client, end to end: snippets with tab stops
//! or plain text, and column items with `labelDetails` that resolve to
//! Markdown documentation.

mod harness;
use harness::jaffle;
use serde_json::{json, Value};

fn snippet(caps: Value) -> Value {
    let project = jaffle();
    let (mut lsp, uri) = project.booted_with(&[], caps);
    let text = "select customer_id from {{ ref('stg_customers') }}\nis_inc";
    lsp.notify(
        "textDocument/didChange",
        json!({"textDocument": {"uri": uri, "version": 2}, "contentChanges": [{"text": text}]}),
    );
    let reply = lsp.request(
        "textDocument/completion",
        json!({"textDocument": {"uri": uri}, "position": {"line": 1, "character": 6}}),
    );
    let item = reply["result"]
        .as_array()
        .expect("items")
        .iter()
        .find(|i| i["label"] == "is_incremental")
        .cloned()
        .unwrap_or_else(|| panic!("no snippet: {reply}"));
    lsp.shutdown();
    item
}

#[test]
fn snippets_follow_the_client_capability() {
    let rich = snippet(json!({"textDocument": {"completion": {"completionItem": {
        "snippetSupport": true, "labelDetailsSupport": true}}}}));
    assert_eq!(rich["insertTextFormat"], 2, "{rich}");
    assert!(
        rich["insertText"].as_str().unwrap().contains("${1:"),
        "{rich}"
    );
    assert_eq!(rich["labelDetails"]["description"], "snippet", "{rich}");

    let plain = snippet(json!({}));
    assert!(plain.get("insertTextFormat").is_none(), "{plain}");
    let text = plain["insertText"].as_str().unwrap();
    assert!(text.starts_with("{% if is_incremental() %}"), "{text}");
    assert!(!text.contains('$'), "{text}");
    assert!(plain.get("labelDetails").is_none(), "{plain}");
}

#[test]
fn column_items_carry_details_and_resolve_to_markdown() {
    let project = jaffle();
    let (mut lsp, uri) = project.booted_with(
        &[],
        json!({"textDocument": {"completion": {"completionItem": {
            "labelDetailsSupport": true, "documentationFormat": ["markdown"]}}}}),
    );
    let text = "select c. from {{ ref('customers') }} c";
    lsp.notify(
        "textDocument/didChange",
        json!({"textDocument": {"uri": uri, "version": 2}, "contentChanges": [{"text": text}]}),
    );
    let reply = lsp.request(
        "textDocument/completion",
        json!({"textDocument": {"uri": uri}, "position": {"line": 0, "character": 9}}),
    );
    let item = reply["result"]
        .as_array()
        .expect("items")
        .iter()
        .find(|i| i["label"] == "customer_id")
        .cloned()
        .unwrap_or_else(|| panic!("{reply}"));
    assert_eq!(item["labelDetails"]["description"], "customers", "{item}");
    let resolved = lsp.request("completionItem/resolve", item);
    let docs = &resolved["result"]["documentation"];
    assert_eq!(docs["kind"], "markdown", "{resolved}");
    let value = docs["value"].as_str().unwrap();
    assert!(value.starts_with("`customer_id`"), "{value}");
    assert!(value.contains("Column of customers"), "{value}");
    assert!(value.contains("unique identifier"), "{value}");
    lsp.shutdown();
}
