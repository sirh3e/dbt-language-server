//! `dbt.queryPlan` end to end (specs/ir-pipeline/09) on per-test copies
//! of the jaffle fixture.

mod harness;
use harness::jaffle;
use serde_json::{json, Value};

fn plan(lsp: &mut harness::Lsp, uri: &str) -> Value {
    lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.queryPlan", "arguments": [{"uri": uri}]}),
    )
}

#[test]
fn query_plan_shows_every_level_for_a_jinja_model() {
    let project = jaffle();
    // customers.sql is what `booted` opens: live text, no compiled file
    let (mut lsp, uri) = project.booted();
    let response = plan(&mut lsp, &uri);
    let result = &response["result"];
    assert_eq!(result["model"], "customers", "{response}");
    assert_eq!(result["backend"], "DuckDb", "{response}");
    assert_ne!(result["status"], "declined", "{response}");
    assert_eq!(result["guidance"], Value::Null, "{response}");

    for level in ["hir", "mir", "lir"] {
        let report = &result["levels"][level];
        assert_eq!(report["available"], true, "{level}: {report}");
        assert_eq!(report["violations"], json!([]), "{level}: {report}");
        assert!(
            report["text"].as_str().is_some_and(|t| !t.is_empty()),
            "{level}: {report}"
        );
    }
    let entries = result["trace"]["entries"].as_array().expect("trace");
    assert!(
        entries.iter().any(|e| e["rule"] == "canon.simplify"),
        "{response}"
    );
    // no manifest or catalog in the copy: the facts the rules need are absent
    assert!(
        entries.iter().any(|e| e["outcome"] == "skipped"
            && e["reason"]
                .as_str()
                .is_some_and(|r| r.starts_with("unmet:"))),
        "{response}"
    );
    assert!(
        result["needs"]["missing"]
            .as_array()
            .is_some_and(|m| m.contains(&json!("phys"))),
        "{response}"
    );

    lsp.shutdown();
}

#[test]
fn query_plan_declines_control_flow_with_guidance_until_compiled_plans() {
    let project = jaffle();
    let (mut lsp, _uri) = project.booted();
    let orders = cogitator_lsp::fs_path_to_uri(&project.root.join("models/orders.sql"));
    // a `{% set %}` + `{% for %}` model, read from disk: not opened
    let response = plan(&mut lsp, &orders);
    let result = &response["result"];
    assert_eq!(result["model"], "orders", "{response}");
    assert_eq!(result["status"], "declined", "{response}");
    let guidance = result["guidance"].as_str().expect("guidance");
    assert!(guidance.contains("dbt compile"), "{guidance}");
    assert_eq!(result["levels"]["hir"]["available"], false, "{response}");
    assert_eq!(result["levels"]["lir"]["available"], false, "{response}");
    assert!(
        result["trace"]["entries"]
            .as_array()
            .is_some_and(|rows| !rows.is_empty()),
        "every rule still reports why it did not run: {response}"
    );

    lsp.shutdown();
}

#[test]
fn query_plan_is_cached_per_generation_and_replans_after_an_edit() {
    let project = jaffle();
    let (mut lsp, uri) = project.booted();
    let probe_uri = cogitator_lsp::fs_path_to_uri(&project.root.join("models/plan_probe.sql"));
    let text = "select id, name from {{ ref('stg_customers') }} where id > 1\n";
    lsp.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": probe_uri, "languageId": "sql", "version": 1, "text": text}}),
    );

    let first = plan(&mut lsp, &probe_uri);
    let second = plan(&mut lsp, &probe_uri);
    assert_eq!(first["result"], second["result"], "same plan");
    let cache = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.cache", "arguments": [{"uri": uri}]}),
    );
    let plans = &cache["result"]["plans"];
    assert_eq!(plans["entries"], 1, "{cache}");
    assert!(plans["hits"].as_u64().is_some_and(|h| h >= 1), "{cache}");

    // an edit bumps the generation: the plan reflects the new text
    lsp.notify(
        "textDocument/didChange",
        json!({"textDocument": {"uri": probe_uri, "version": 2},
            "contentChanges": [{"text": "select id from {{ ref('stg_customers') }}\n"}]}),
    );
    let after = plan(&mut lsp, &probe_uri);
    let mir = after["result"]["levels"]["mir"]["text"]
        .as_str()
        .expect("mir");
    assert!(!mir.contains("name"), "the edited text was planned: {mir}");

    lsp.shutdown();
}

#[test]
fn query_plan_rejects_a_non_model_uri_with_guidance() {
    let project = jaffle();
    let (mut lsp, _uri) = project.booted();
    let schema = cogitator_lsp::fs_path_to_uri(&project.root.join("models/schema.yml"));
    let response = plan(&mut lsp, &schema);
    let message = response["error"]["message"].as_str().expect("error");
    assert!(message.contains("needs a model"), "{message}");
    lsp.shutdown();
}
