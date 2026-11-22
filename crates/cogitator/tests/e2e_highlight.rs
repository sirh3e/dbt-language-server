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
