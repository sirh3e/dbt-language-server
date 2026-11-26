//! The model-aware snippets end to end: a client with snippet support
//! gets `insertTextFormat: 2` and tab stops, one without gets plain text.

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
