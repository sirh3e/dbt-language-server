//! End-to-end suite (specs/rust-port/05 §7) — rename of a model's output
//! column over the jaffle fixture: prepareRename, the transitive edit
//! (stg_customers → customers → the YAML that names both), references
//! that agree with it, and a refusal naming the file the lineage cannot
//! read.

mod harness;
use harness::jaffle;

fn position_of(text: &str, needle: &str, offset: u32) -> serde_json::Value {
    let at = text.find(needle).expect("needle");
    let line = text[..at].matches('\n').count();
    let col = at - text[..at].rfind('\n').map_or(0, |i| i + 1);
    serde_json::json!({"line": line, "character": col as u32 + offset})
}

fn open(lsp: &mut harness::Lsp, path: &std::path::Path) -> (String, String) {
    let text = std::fs::read_to_string(path).unwrap();
    let uri = cogitator_lsp::fs_path_to_uri(path);
    lsp.notify(
        "textDocument/didOpen",
        serde_json::json!({"textDocument": {"uri": uri, "languageId": "sql", "version": 1,
            "text": text}}),
    );
    (uri, text)
}

#[test]
fn a_column_rename_reaches_every_consumer_or_refuses() {
    let project = jaffle();
    let (mut lsp, _) = project.booted();
    let (uri, text) = open(
        &mut lsp,
        &project.root.join("models/staging/stg_customers.sql"),
    );
    let at = position_of(&text, "as customer_id", 4);

    let prep = lsp.request(
        "textDocument/prepareRename",
        serde_json::json!({"textDocument": {"uri": uri}, "position": at}),
    );
    assert_eq!(prep["result"]["placeholder"], "customer_id", "{prep}");

    let edit = lsp.request(
        "textDocument/rename",
        serde_json::json!({"textDocument": {"uri": uri}, "position": at, "newName": "cust_id"}),
    );
    let changes = edit["result"]["documentChanges"]
        .as_array()
        .unwrap_or_else(|| panic!("{edit}"));
    let edits_of = |file: &str| -> Vec<String> {
        changes
            .iter()
            .filter(|c| {
                c["textDocument"]["uri"]
                    .as_str()
                    .is_some_and(|u| u.ends_with(file))
            })
            .flat_map(|c| c["edits"].as_array().cloned().unwrap_or_default())
            .map(|e| e["newText"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(edits_of("stg_customers.sql"), ["cust_id"], "{edit}");
    // customers.sql: `customers.customer_id` in the select and both joins
    assert_eq!(edits_of("/customers.sql"), ["cust_id"; 3], "{edit}");
    // stg_customers' entry; customers' entry (it outputs the column on
    // through select *) and orders' relationships `field:` to it
    assert_eq!(edits_of("staging/schema.yml"), ["cust_id"], "{edit}");
    assert_eq!(edits_of("models/schema.yml"), ["cust_id"; 2], "{edit}");

    let refs = lsp.request(
        "textDocument/references",
        serde_json::json!({"textDocument": {"uri": uri}, "position": at,
            "context": {"includeDeclaration": true}}),
    );
    let count = refs["result"].as_array().map_or(0, Vec::len);
    let edits: usize = changes
        .iter()
        .map(|c| c["edits"].as_array().map_or(0, Vec::len))
        .sum();
    assert_eq!(count, edits, "references agree with the rename: {refs}");

    // stg_orders.status: orders.sql's Jinja loops hide what it reads
    let (orders_uri, orders_text) = open(
        &mut lsp,
        &project.root.join("models/staging/stg_orders.sql"),
    );
    let refused = lsp.request(
        "textDocument/rename",
        serde_json::json!({"textDocument": {"uri": orders_uri},
            "position": position_of(&orders_text, "status", 1), "newName": "state"}),
    );
    assert_eq!(refused["error"]["code"], -32803, "{refused}");
    let message = refused["error"]["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("orders.sql") && message.contains("control blocks"),
        "{refused}"
    );
}
