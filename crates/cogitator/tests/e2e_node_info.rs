//! `dbt.nodeInfo` end to end (specs/highlighting §6.1): the card data of a
//! macro, a singular test, a generic test and a model on the jaffle fixture.

mod harness;
use harness::jaffle;
use serde_json::{json, Value};

fn node_info(lsp: &mut harness::client::Lsp, kind: &str, name: &str) -> Value {
    lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.nodeInfo", "arguments": [{"node": {"kind": kind, "name": name}}]}),
    )
}

fn names(list: &Value) -> Vec<String> {
    list.as_array()
        .map(|a| {
            a.iter()
                .map(|e| e["name"].as_str().unwrap_or("").to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_macro_says_who_calls_it_and_its_signature() {
    let project = jaffle();
    let (mut lsp, _uri) = project.booted();
    let response = node_info(&mut lsp, "macro", "times_five");
    let info = &response["result"];
    assert_eq!(info["kind"], "macro", "{response}");
    assert_eq!(info["params"][0]["name"], "int_value", "{response}");
    assert!(
        info["uri"]
            .as_str()
            .is_some_and(|u| u.ends_with("jaffle_macros.sql")),
        "{response}"
    );
    assert!(
        names(&info["callers"]).contains(&"customers".to_string()),
        "{response}"
    );
    assert!(info["range"]["start"]["line"].is_u64(), "{response}");
}

#[test]
fn a_singular_test_says_what_it_refs() {
    let project = jaffle();
    let (mut lsp, _uri) = project.booted();
    let response = node_info(&mut lsp, "test", "assert_positive_order_amount");
    let info = &response["result"];
    assert_eq!(info["kind"], "test", "{response}");
    assert_eq!(names(&info["refs"]), vec!["orders"], "{response}");
    assert!(
        info["compiledUri"].is_null(),
        "not compiled yet: {response}"
    );
    assert!(
        info["downstream"].as_array().is_some_and(|d| d.is_empty()),
        "{response}"
    );
}

#[test]
fn a_generic_test_says_where_it_is_applied() {
    let project = jaffle();
    std::fs::write(
        project.root.join("macros/not_negative.sql"),
        "{% test not_negative(model, column_name) %}\nselect * from {{ model }} where {{ column_name }} < 0\n{% endtest %}\n",
    )
    .unwrap();
    let schema = project.root.join("models/schema.yml");
    // the checkout may have CRLF line endings (Windows)
    let text = std::fs::read_to_string(&schema)
        .unwrap()
        .replace("\r\n", "\n");
    let anchor = "          - unique\n          - not_null\n";
    assert!(
        text.contains(anchor),
        "the fixture's customer_id tests moved"
    );
    let text = text.replacen(
        anchor,
        "          - unique\n          - not_null\n          - not_negative\n",
        1,
    );
    std::fs::write(&schema, text).unwrap();
    let (mut lsp, _uri) = project.booted();
    let response = node_info(&mut lsp, "generic_test", "not_negative");
    let info = &response["result"];
    assert_eq!(info["kind"], "generic_test", "{response}");
    assert_eq!(
        info["params"],
        json!(["model", "column_name"]),
        "{response}"
    );
    assert_eq!(info["range"]["start"]["line"], 0, "{response}");
    let applied = info["appliedTo"].as_array().expect("appliedTo");
    assert_eq!(applied.len(), 1, "{response}");
    assert_eq!(applied[0]["model"], "customers");
    assert_eq!(applied[0]["column"], "customer_id");
}

#[test]
fn a_model_is_its_model_info_with_a_kind() {
    let project = jaffle();
    let (mut lsp, _uri) = project.booted();
    let response = node_info(&mut lsp, "model", "customers");
    assert_eq!(response["result"]["kind"], "model", "{response}");
    assert_eq!(response["result"]["model"], "customers", "{response}");
}

#[test]
fn an_unknown_node_is_null_with_a_suggestion() {
    let project = jaffle();
    let (mut lsp, _uri) = project.booted();
    let response = node_info(&mut lsp, "macro", "times_fiv");
    assert!(response["result"].is_null(), "{response}");
    let warning = lsp.wait_notification("window/showMessage", |m| {
        m["params"]["message"]
            .as_str()
            .is_some_and(|t| t.contains("times_five"))
    });
    assert!(warning["params"]["message"]
        .as_str()
        .unwrap()
        .contains("times_fiv"));
    let bad = node_info(&mut lsp, "widget", "x");
    assert!(
        bad["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("widget")),
        "{bad}"
    );
}
