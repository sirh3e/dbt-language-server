//! End-to-end suite (specs/rust-port/05 §7) — this file's theme only;
//! the framed-LSP harness lives in tests/harness/.

mod harness;
use harness::jaffle;
use serde_json::json;

const CATALOG: &str = r#"{"metadata": {"generated_at": "2026-11-20T10:00:00Z"},
    "nodes": {"model.jaffle_shop.stg_customers": {
        "metadata": {"type": "VIEW", "schema": "main", "name": "stg_customers"},
        "columns": {
            "customer_id": {"name": "customer_id", "type": "INTEGER", "index": 1},
            "first_name": {"name": "first_name", "type": "VARCHAR", "index": 2},
            "last_name": {"name": "last_name", "type": "VARCHAR", "index": 3}}}},
    "sources": {}}"#;

fn breaking_items(params: &serde_json::Value) -> Vec<serde_json::Value> {
    params["items"]
        .as_array()
        .or_else(|| params["diagnostics"].as_array())
        .map(|items| {
            items
                .iter()
                .filter(|d| d["code"] == "breaking-change")
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

/// A saved rename of a tested column against target/catalog.json: the
/// save republishes an error diagnostic, the lens counts it, the
/// command reports the diff and its downstream reads, dbt.status names
/// the baseline.
#[test]
fn a_saved_rename_is_a_breaking_change() {
    let project = jaffle();
    let root = project.root.clone();
    std::fs::create_dir_all(root.join("target")).unwrap();
    std::fs::write(root.join("target/catalog.json"), CATALOG).unwrap();
    let model = root.join("models/staging/stg_customers.sql");
    let original = std::fs::read_to_string(&model).unwrap();
    let uri = cogitator_lsp::fs_path_to_uri(&model);

    let (mut lsp, _) = project.booted();
    lsp.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": uri, "languageId": "sql", "version": 1, "text": original}}),
    );

    // the edit lands on disk, then the save
    let edited = original.replace("id as customer_id", "id as customer_key");
    assert_ne!(edited, original, "the fixture still renames id");
    std::fs::write(&model, &edited).unwrap();
    lsp.notify(
        "textDocument/didChange",
        json!({"textDocument": {"uri": uri, "version": 2},
               "contentChanges": [{"text": edited}]}),
    );
    lsp.notify(
        "textDocument/didSave",
        json!({"textDocument": {"uri": uri}}),
    );
    let published = lsp.wait_notification("textDocument/publishDiagnostics", |n| {
        n["params"]["uri"] == uri && !breaking_items(&n["params"]).is_empty()
    });
    let items = breaking_items(&published["params"]);
    assert_eq!(items.len(), 1, "{items:?}");
    let message = items[0]["message"].as_str().unwrap();
    assert!(message.contains("'customer_id' was removed"), "{message}");
    assert!(
        message.contains("possibly renamed to 'customer_key'"),
        "{message}"
    );
    assert!(message.contains("tested: unique, not_null"), "{message}");
    assert_eq!(
        items[0]["severity"], 1,
        "a tested column removed is an error"
    );

    // the pull path serves the same cached finding
    let pulled = lsp.request(
        "textDocument/diagnostic",
        json!({"textDocument": {"uri": uri}}),
    );
    assert_eq!(breaking_items(&pulled["result"]).len(), 1, "{pulled}");

    // the lens counts it and runs the command
    let lenses = lsp.request(
        "textDocument/codeLens",
        json!({"textDocument": {"uri": uri}}),
    );
    let lens = lenses["result"]
        .as_array()
        .and_then(|l| {
            l.iter()
                .find(|l| l["command"]["command"] == "dbt.breakingChanges")
                .cloned()
        })
        .expect("breaking lens");
    assert_eq!(lens["command"]["title"], "1 breaking change");

    // the command: the diff and what reads the column downstream
    let report = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.breakingChanges", "arguments": [{"uri": uri}]}),
    );
    let report = &report["result"];
    assert_eq!(report["status"], "compared", "{report}");
    assert_eq!(report["baseline"]["source"], "catalog");
    assert_eq!(report["diff"]["removed"][0]["name"], "customer_id");
    assert_eq!(
        report["diff"]["removed"][0]["possiblyRenamedTo"],
        "customer_key"
    );
    assert_eq!(report["diff"]["added"][0]["name"], "customer_key");
    assert_eq!(report["impacts"][0]["severity"], "error");
    let read = &report["impacts"][0]["downstream"][0];
    assert_eq!(read["model"], "customers", "{report}");
    assert_eq!(read["column"], "customer_id");

    // dbt.status names the baseline the preview uses
    let status = lsp.wait_status(|s| s["breaking_changes"]["baseline"] == "catalog");
    assert_eq!(status["breaking_changes"]["baseline"], "catalog");
}

/// Without target/ artifacts the preview is silent: no diagnostic, no
/// lens, and the command and dbt.status say why.
#[test]
fn no_baseline_is_silence() {
    let project = jaffle();
    let root = project.root.clone();
    let model = root.join("models/staging/stg_customers.sql");
    let text = std::fs::read_to_string(&model).unwrap();
    let uri = cogitator_lsp::fs_path_to_uri(&model);
    let (mut lsp, _) = project.booted();
    lsp.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": uri, "languageId": "sql", "version": 1, "text": text}}),
    );
    let report = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.breakingChanges", "arguments": [{"uri": uri}]}),
    );
    assert_eq!(report["result"]["status"], "noBaseline", "{report}");
    let pulled = lsp.request(
        "textDocument/diagnostic",
        json!({"textDocument": {"uri": uri}}),
    );
    assert!(breaking_items(&pulled["result"]).is_empty(), "{pulled}");
    let status = lsp.wait_status(|s| s["breaking_changes"]["baseline"] == "none");
    assert!(
        status["breaking_changes"]["note"]
            .as_str()
            .is_some_and(|n| n.contains("silent")),
        "{status}"
    );
}

/// The inferred output-column types feed the preview: a saved cast of
/// `customer_id` to text against the catalog's INTEGER is a type change.
/// The types come from the project schema snapshot, which the refresh
/// after boot publishes, so the command is asked until it has them.
#[test]
fn an_inferred_type_change_is_reported() {
    let project = jaffle();
    let root = project.root.clone();
    std::fs::create_dir_all(root.join("target")).unwrap();
    std::fs::write(root.join("target/catalog.json"), CATALOG).unwrap();
    let model = root.join("models/staging/stg_customers.sql");
    let original = std::fs::read_to_string(&model).unwrap();
    let edited = original.replace("id as customer_id", "cast(id as varchar) as customer_id");
    assert_ne!(
        edited, original,
        "the fixture still selects id as customer_id"
    );
    std::fs::write(&model, &edited).unwrap();
    let uri = cogitator_lsp::fs_path_to_uri(&model);

    let (mut lsp, _) = project.booted();
    lsp.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": uri, "languageId": "sql", "version": 1, "text": edited}}),
    );
    let report = lsp.wait_until("the cast of customer_id is a type change", |lsp| {
        let reply = lsp.request(
            "workspace/executeCommand",
            json!({"command": "dbt.breakingChanges", "arguments": [{"uri": uri}]}),
        );
        let retyped = reply["result"]["diff"]["retyped"].as_array()?;
        (!retyped.is_empty()).then(|| reply["result"].clone())
    });
    let change = &report["diff"]["retyped"][0];
    assert_eq!(change["name"], "customer_id", "{report}");
    assert_eq!(change["from"], "INTEGER", "{report}");
    assert_eq!(change["to"], "varchar", "{report}");
    assert!(
        report["diff"]["removed"].as_array().unwrap().is_empty(),
        "{report}"
    );
}
