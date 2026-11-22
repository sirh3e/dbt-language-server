//! `dbt.highlight` end to end: tree-sitter-dbt's highlights of the open
//! model, of a text, and of a model's compiled SQL, as the Neovim
//! explorer paints its dbt SQL and compiled SQL panes.

mod harness;
use harness::jaffle;
use serde_json::{json, Value};

fn captures(response: &Value) -> Vec<String> {
    response["result"]["spans"]
        .as_array()
        .unwrap_or_else(|| panic!("spans: {response}"))
        .iter()
        .map(|s| s["capture"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn the_open_model_is_highlighted_by_the_dbt_grammar() {
    let project = jaffle();
    let (mut lsp, uri) = project.booted();
    let response = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.highlight", "arguments": [{"uri": uri}]}),
    );
    assert_eq!(response["result"]["language"], "dbt", "{response}");
    assert_eq!(response["result"]["truncated"], false, "{response}");
    let found = captures(&response);
    assert!(
        found.iter().any(|c| c == "tag.delimiter"),
        "{{{{ }}}}: {response}"
    );
    assert!(
        found.iter().any(|c| c.starts_with("keyword")),
        "SQL keywords: {response}"
    );
    let first = &response["result"]["spans"][0]["range"];
    assert!(
        first["start"]["line"].is_u64() && first["end"]["character"].is_u64(),
        "{response}"
    );
}

#[test]
fn a_text_is_highlighted_in_the_language_it_names() {
    let project = jaffle();
    let (mut lsp, _uri) = project.booted();
    let response = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.highlight", "arguments": [{"text": "select 1 from t", "language": "sql"}]}),
    );
    assert_eq!(response["result"]["language"], "sql", "{response}");
    assert!(
        captures(&response).iter().any(|c| c.starts_with("keyword")),
        "{response}"
    );

    let response = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.highlight", "arguments": [{"text": "x", "language": "python"}]}),
    );
    assert!(
        response["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("python")),
        "{response}"
    );
}

#[test]
fn a_model_dbt_has_not_compiled_answers_null() {
    let project = jaffle();
    let (mut lsp, uri) = project.booted();
    let response = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.highlight", "arguments": [{"uri": uri, "compiled": true}]}),
    );
    assert!(response["result"].is_null(), "{response}");
}

// ---- dbt.highlight v2 (specs/highlighting §2) ----

fn spans(response: &Value) -> Vec<Value> {
    response["result"]["spans"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

#[test]
fn a_yaml_file_is_known_by_its_uri_and_coloured_by_every_layer() {
    let project = jaffle();
    let (mut lsp, _uri) = project.booted();
    let schema = cogitator_lsp::fs_path_to_uri(&project.root.join("models/schema.yml"));
    let response = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.highlight", "arguments": [{"uri": schema, "version": 7}]}),
    );
    let result = &response["result"];
    assert_eq!(result["language"], "yaml", "{response}");
    assert_eq!(result["version"], 7, "the version is echoed: {response}");
    assert_eq!(
        result["layers"],
        json!(["yaml", "sql", "jinja", "dbt"]),
        "{response}"
    );
    let found = spans(&response);
    let has = |capture: &str, layer: &str| {
        found
            .iter()
            .any(|s| s["capture"] == capture && s["layer"] == layer)
    };
    assert!(has("property", "yaml"), "{response}");
    assert!(has("type.dbt.model", "dbt"), "models' names: {response}");
    assert!(has("property.dbt.column", "dbt"), "{response}");
    assert!(has("function.dbt.test", "dbt"), "{response}");
    assert!(
        has("tag.delimiter", "jinja"),
        "the doc() of a description: {response}"
    );
    // orders_status is a doc block of the project: resolved
    assert!(has("string.special.dbt.doc", "dbt"), "{response}");
}

#[test]
fn a_range_answers_the_spans_of_those_lines_only() {
    let project = jaffle();
    let (mut lsp, uri) = project.booted();
    let all = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.highlight", "arguments": [{"uri": uri}]}),
    );
    let some = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.highlight", "arguments": [{"uri": uri,
            "range": {"start": {"line": 2}, "end": {"line": 3}}}]}),
    );
    let (all, some) = (spans(&all), spans(&some));
    assert!(
        !some.is_empty() && some.len() < all.len(),
        "{} of {}",
        some.len(),
        all.len()
    );
    assert!(
        some.iter().all(|s| s["range"]["start"]["line"] == 2),
        "{some:?}"
    );
}

#[test]
fn a_repeat_is_served_from_the_cache() {
    let project = jaffle();
    let (mut lsp, uri) = project.booted();
    let ask = |lsp: &mut harness::client::Lsp| {
        lsp.request(
            "workspace/executeCommand",
            json!({"command": "dbt.highlight", "arguments": [{"uri": uri}]}),
        )
    };
    let first = ask(&mut lsp);
    let second = ask(&mut lsp);
    assert_eq!(first["result"], second["result"]);
    let cache = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.cache", "arguments": [{"uri": uri}]}),
    );
    let highlights = &cache["result"]["highlights"];
    assert!(highlights["hits"].as_u64() >= Some(1), "{cache}");
    assert!(highlights["entries"].as_u64() >= Some(1), "{cache}");
}

#[test]
fn a_node_is_highlighted_from_its_file() {
    let project = jaffle();
    let (mut lsp, _uri) = project.booted();
    let ask = |lsp: &mut harness::client::Lsp, node: Value| {
        lsp.request(
            "workspace/executeCommand",
            json!({"command": "dbt.highlight", "arguments": [node]}),
        )
    };
    let macro_ = ask(
        &mut lsp,
        json!({"node": {"kind": "macro", "name": "times_five"}}),
    );
    assert_eq!(macro_["result"]["language"], "dbt", "{macro_}");
    assert!(
        spans(&macro_)
            .iter()
            .any(|s| s["capture"] == "keyword.function"),
        "{{% macro %}}: {macro_}"
    );
    let test = ask(
        &mut lsp,
        json!({"node": {"kind": "test", "name": "assert_positive_order_amount"}}),
    );
    assert!(spans(&test).iter().any(|s| s["layer"] == "sql"), "{test}");
    // a macro has no compiled SQL; an uncompiled test neither
    let compiled = ask(
        &mut lsp,
        json!({"node": {"kind": "macro", "name": "times_five"}, "compiled": true}),
    );
    assert!(compiled["result"].is_null(), "{compiled}");
    let missing = ask(&mut lsp, json!({"node": {"kind": "macro", "name": "nope"}}));
    assert!(
        missing["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("nope")),
        "{missing}"
    );
}

#[test]
fn a_compiled_test_is_highlighted_as_sql() {
    let project = jaffle();
    let compiled = project
        .root
        .join("target/compiled/jaffle_shop/tests/assert_positive_order_amount.sql");
    std::fs::create_dir_all(compiled.parent().unwrap()).unwrap();
    std::fs::write(
        &compiled,
        "select order_id from main.orders where amount < 0\n",
    )
    .unwrap();
    let (mut lsp, _uri) = project.booted();
    let response = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.highlight", "arguments": [{"node": {"kind": "test", "name": "assert_positive_order_amount"}, "compiled": true}]}),
    );
    assert_eq!(response["result"]["language"], "sql", "{response}");
    assert!(
        spans(&response).iter().all(|s| s["layer"] == "sql"),
        "{response}"
    );
}
