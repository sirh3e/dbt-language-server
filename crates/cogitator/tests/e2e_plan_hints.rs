//! Plan inlay hints (catalog-proven CTE row counts) and dbt.incrementalDiff
//! with its code lens, end to end on per-test copies of the jaffle fixture.

mod harness;
use harness::{jaffle, Lsp};
use serde_json::{json, Value};

const PROBE: &str = "with recent as (\n    select order_id from {{ ref('stg_orders') }}\n),\n\
                     open_orders as (\n    select order_id from recent where order_id > 3\n)\n\
                     select order_id from open_orders\n";

const CATALOG: &str = r#"{"metadata": {"generated_at": "2026-11-20T10:00:00Z"},
    "nodes": {"model.jaffle_shop.stg_orders": {
        "metadata": {"type": "BASE TABLE", "schema": "main", "name": "stg_orders"},
        "columns": {"order_id": {"type": "INTEGER", "index": 1, "name": "order_id"}},
        "stats": {"row_count": {"id": "row_count", "value": 1200, "include": true}}}},
    "sources": {}}"#;

const EVENTS: &str = "{{ config(materialized='incremental') }}\n\
                      select order_id from {{ ref('stg_orders') }}\n\
                      {% if is_incremental() %}\n\
                      where order_id > (select max(order_id) from {{ this }})\n\
                      {% endif %}\n";

fn open(lsp: &mut Lsp, uri: &str, text: &str) {
    lsp.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": uri, "languageId": "sql", "version": 1, "text": text}}),
    );
}

fn labels(lsp: &mut Lsp, uri: &str) -> Vec<String> {
    let reply = lsp.request(
        "textDocument/inlayHint",
        json!({"textDocument": {"uri": uri},
               "range": {"start": {"line": 0, "character": 0}, "end": {"line": 99, "character": 0}}}),
    );
    reply["result"]
        .as_array()
        .map(|hints| {
            hints
                .iter()
                .filter_map(|h| h["label"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn cte_row_counts_show_once_the_catalog_has_statistics() {
    let project = jaffle();
    let target = project.root.join("target");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(target.join("catalog.json"), CATALOG).unwrap();
    let path = project.root.join("models/row_probe.sql");
    std::fs::write(&path, PROBE).unwrap();
    let uri = cogitator_lsp::fs_path_to_uri(&path);
    let (mut lsp, _) = project.booted();
    open(&mut lsp, &uri, PROBE);
    let shown = lsp.wait_until("row hints after the CTE names", |lsp| {
        let labels = labels(lsp, &uri);
        labels
            .iter()
            .any(|l| l.ends_with(" rows"))
            .then_some(labels)
    });
    assert!(shown.contains(&"1,200 rows".to_string()), "{shown:?}");
    assert!(shown.contains(&"≤ 1,200 rows".to_string()), "{shown:?}");
    lsp.shutdown();
}

#[test]
fn row_hints_are_absent_without_statistics_or_when_switched_off() {
    let project = jaffle();
    let path = project.root.join("models/row_probe.sql");
    std::fs::write(&path, PROBE).unwrap();
    let uri = cogitator_lsp::fs_path_to_uri(&path);
    let (mut lsp, _) = project.booted();
    open(&mut lsp, &uri, PROBE);
    // no catalog.json: nothing, rather than a guess
    let shown = labels(&mut lsp, &uri);
    assert!(shown.iter().all(|l| !l.ends_with(" rows")), "{shown:?}");
    lsp.shutdown();

    let project = jaffle();
    let target = project.root.join("target");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(target.join("catalog.json"), CATALOG).unwrap();
    std::fs::write(
        project.root.join("cogitator.yml"),
        "perf:\n  row_hints: false\n",
    )
    .unwrap();
    let path = project.root.join("models/row_probe.sql");
    std::fs::write(&path, PROBE).unwrap();
    let uri = cogitator_lsp::fs_path_to_uri(&path);
    let (mut lsp, _) = project.booted();
    open(&mut lsp, &uri, PROBE);
    lsp.wait_until("the config file is loaded", |lsp| {
        let status = lsp.request(
            "workspace/executeCommand",
            json!({"command": "dbt.status", "arguments": [{}]}),
        );
        (status["result"]["perf"]["rowHints"] == Value::Bool(false)).then_some(())
    });
    let shown = labels(&mut lsp, &uri);
    assert!(shown.iter().all(|l| !l.ends_with(" rows")), "{shown:?}");
    lsp.shutdown();
}

#[test]
fn incremental_diff_answers_and_its_lens_opens_the_diff() {
    let project = jaffle();
    let path = project.root.join("models/events_incr.sql");
    std::fs::write(&path, EVENTS).unwrap();
    let uri = cogitator_lsp::fs_path_to_uri(&path);
    let (mut lsp, _) = project.booted();
    open(&mut lsp, &uri, EVENTS);

    let lenses = lsp.wait_until("the incremental lens", |lsp| {
        let reply = lsp.request(
            "textDocument/codeLens",
            json!({"textDocument": {"uri": uri}}),
        );
        let lenses = reply["result"].as_array().cloned().unwrap_or_default();
        lenses
            .iter()
            .any(|l| l["command"]["command"] == "dbt.incrementalDiff")
            .then_some(lenses)
    });
    let lens = lenses
        .iter()
        .find(|l| l["command"]["command"] == "dbt.incrementalDiff")
        .unwrap();
    assert_eq!(
        lens["command"]["title"],
        "Compare incremental / full refresh"
    );

    let reply = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.incrementalDiff", "arguments": [{"uri": uri}]}),
    );
    let result = &reply["result"];
    assert_eq!(result["model"], "events_incr", "{reply}");
    assert_eq!(result["identical"], false, "{reply}");
    let diff = result["diff"].as_str().expect("diff");
    assert!(
        diff.contains("+where order_id > (select max(order_id) from "),
        "{diff}"
    );
    assert!(
        !result["fullRefresh"].as_str().unwrap().contains("where"),
        "{reply}"
    );

    // the lens argument: the diff opens as a document
    let _ = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.incrementalDiff", "arguments": [lens["command"]["arguments"][0]]}),
    );
    let shown = lsp.wait_server_request("window/showDocument", |m| {
        m["params"]["uri"]
            .as_str()
            .is_some_and(|u| u.ends_with("events_incr.diff"))
    });
    let shown_path = cogitator_lsp::uri_to_fs_path(shown["params"]["uri"].as_str().unwrap())
        .expect("a file uri");
    let written = std::fs::read_to_string(shown_path).expect("the diff file");
    assert!(written.contains("(incremental)"), "{written}");

    // a table model gets no such lens
    let customers = cogitator_lsp::fs_path_to_uri(&project.root.join("models/customers.sql"));
    let reply = lsp.request(
        "textDocument/codeLens",
        json!({"textDocument": {"uri": customers}}),
    );
    let none = reply["result"].as_array().cloned().unwrap_or_default();
    assert!(
        none.iter()
            .all(|l| l["command"]["command"] != "dbt.incrementalDiff"),
        "{none:?}"
    );
    lsp.shutdown();
}
