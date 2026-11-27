//! The authoring actions through textDocument/codeAction on the booted
//! server, round trip: the staging scaffold's files are written as the
//! client would apply them, the server learns of them (didCreateFiles),
//! and the action is no longer offered; inline-model carries its
//! DeleteFile; a select item inherits its upstream description.

mod harness;
use harness::{jaffle, Lsp};

fn refreshes(lsp: &mut Lsp) -> u64 {
    lsp.wait_status(|_| true)["refresh"]["count"]
        .as_u64()
        .unwrap_or(0)
}

fn open(lsp: &mut Lsp, uri: &str, text: &str) {
    lsp.notify(
        "textDocument/didOpen",
        serde_json::json!({"textDocument": {"uri": uri, "languageId": "sql", "version": 1,
            "text": text}}),
    );
}

fn actions_at(lsp: &mut Lsp, uri: &str, line: u32, character: u32) -> Vec<serde_json::Value> {
    let reply = lsp.request(
        "textDocument/codeAction",
        serde_json::json!({"textDocument": {"uri": uri},
                           "range": {"start": {"line": line, "character": character},
                                     "end": {"line": line, "character": character}},
                           "context": {"diagnostics": []}}),
    );
    reply["result"].as_array().cloned().unwrap_or_default()
}

fn titled<'a>(actions: &'a [serde_json::Value], prefix: &str) -> Option<&'a serde_json::Value> {
    actions
        .iter()
        .find(|a| a["title"].as_str().is_some_and(|t| t.starts_with(prefix)))
}

/// Apply a WorkspaceEdit of created files to disk, as a client would.
fn apply_creations(edit: &serde_json::Value) -> Vec<String> {
    let mut created = Vec::new();
    for change in edit["documentChanges"].as_array().unwrap() {
        if change["kind"] == "create" {
            created.push(change["uri"].as_str().unwrap().to_string());
        } else if change.get("kind").is_none() {
            let uri = change["textDocument"]["uri"].as_str().unwrap();
            if created.iter().any(|c| c == uri) {
                let path = cogitator_lsp::uri_to_fs_path(uri).unwrap();
                let text = change["edits"][0]["newText"].as_str().unwrap();
                std::fs::write(path, text).unwrap();
            }
        }
    }
    created
}

#[test]
fn authoring_actions_round_trip() {
    let project = jaffle();
    let root = project.root.clone();
    std::fs::write(root.join("models/probe_lonely.sql"), "select 1 as n\n").unwrap();
    let inline_text = "select n from {{ ref('probe_lonely') }}\n";
    std::fs::write(root.join("models/probe_inline.sql"), inline_text).unwrap();
    let relay_text = "select customer_id, order_count from {{ ref('orders') }}\n";
    std::fs::write(root.join("models/probe_relay.sql"), relay_text).unwrap();
    let (mut lsp, _uri) = project.booted();
    let uri = |rel: &str| cogitator_lsp::fs_path_to_uri(&root.join(rel));

    // 1. scaffold a staging model over a seed ref, apply it, and it is gone
    let stg = uri("models/staging/stg_orders.sql");
    let stg_text = std::fs::read_to_string(root.join("models/staging/stg_orders.sql")).unwrap();
    open(&mut lsp, &stg, &stg_text);
    let (line, col) = stg_text
        .lines()
        .enumerate()
        .find_map(|(i, l)| l.find("raw_orders").map(|c| (i as u32, c as u32)))
        .unwrap();
    let actions = actions_at(&mut lsp, &stg, line, col);
    let scaffold = titled(&actions, "Scaffold staging model stg_raw_orders")
        .unwrap_or_else(|| panic!("scaffold action in {actions:#?}"))
        .clone();
    assert_eq!(scaffold["kind"], "refactor", "{scaffold}");
    let created = apply_creations(&scaffold["edit"]);
    assert_eq!(created.len(), 2, "{scaffold}");
    let sql = std::fs::read_to_string(root.join("models/staging/stg_raw_orders.sql")).unwrap();
    assert!(
        sql.contains("select * from {{ ref('raw_orders') }}"),
        "{sql}"
    );
    assert!(sql.contains("select * from renamed"), "{sql}");
    let before = refreshes(&mut lsp);
    lsp.notify(
        "workspace/didCreateFiles",
        serde_json::json!({"files": created.iter().map(|u| serde_json::json!({"uri": u})).collect::<Vec<_>>()}),
    );
    lsp.wait_refresh(before + 1);
    let actions = actions_at(&mut lsp, &stg, line, col);
    assert!(
        titled(&actions, "Scaffold staging model stg_raw_orders").is_none(),
        "offered again after it was written: {actions:#?}"
    );

    // 2. inline a small model: the edit replaces the ref and deletes the file
    let inline = uri("models/probe_inline.sql");
    open(&mut lsp, &inline, inline_text);
    let actions = actions_at(&mut lsp, &inline, 0, 23);
    let action = titled(&actions, "Inline model 'probe_lonely' as a CTE")
        .unwrap_or_else(|| panic!("inline action in {actions:#?}"));
    let changes = action["edit"]["documentChanges"].as_array().unwrap();
    assert!(
        changes
            .iter()
            .any(|c| c["kind"] == "delete" && c["uri"] == uri("models/probe_lonely.sql")),
        "{action}"
    );
    let edits = changes[0]["edits"].as_array().unwrap();
    assert!(
        edits.iter().any(|e| e["newText"]
            .as_str()
            .is_some_and(|t| t.starts_with("with probe_lonely as ("))),
        "{action}"
    );

    // 3. a select item inherits its upstream description into a new file
    let relay = uri("models/probe_relay.sql");
    open(&mut lsp, &relay, relay_text);
    let actions = actions_at(&mut lsp, &relay, 0, 8);
    let inherit = titled(
        &actions,
        "Inherit description of 'customer_id' from orders.customer_id",
    )
    .unwrap_or_else(|| panic!("inherit action in {actions:#?}"));
    let changes = inherit["edit"]["documentChanges"].as_array().unwrap();
    assert_eq!(changes[0]["kind"], "create", "{inherit}");
    assert!(
        changes[1]["edits"][0]["newText"]
            .as_str()
            .is_some_and(|t| t.contains("- name: customer_id") && t.contains("description:")),
        "{inherit}"
    );

    lsp.shutdown();
}
