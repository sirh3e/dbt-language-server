//! End-to-end suite (specs/rust-port/05 §7) — this file's theme only:
//! properties YAML — schema-driven completion, tests on unproduced
//! columns, and "Add missing columns" offered from the YAML entry.

mod harness;
use harness::{jaffle, Lsp};

const SQL: &str = "select customer_id, first_name, last_name from {{ ref('customers') }}\n";
const YAML: &str = "version: 2\n\nmodels:\n  - name: probe_props\n    config:\n      materialized: table\n    columns:\n      - name: customer_id\n        data_tests:\n          - not_null\n      - name: frist_name\n        data_tests:\n          - unique\n";
const TEST_MACRO: &str =
    "{% test positive(model, column_name, strict=true) %}\nselect 1\n{% endtest %}\n";

fn labels_at(lsp: &mut Lsp, uri: &str, line: u32, character: u32) -> Vec<String> {
    let reply = lsp.request(
        "textDocument/completion",
        serde_json::json!({"textDocument": {"uri": uri},
                           "position": {"line": line, "character": character}}),
    );
    reply["result"]
        .as_array()
        .unwrap_or_else(|| panic!("items: {reply}"))
        .iter()
        .filter_map(|i| i["label"].as_str().map(str::to_string))
        .collect()
}

#[test]
fn properties_yaml_completion_diagnostics_and_actions() {
    let project = jaffle();
    let root = project.root.clone();
    std::fs::write(root.join("models/probe_props.sql"), SQL).unwrap();
    std::fs::write(root.join("models/probe_props.yml"), YAML).unwrap();
    std::fs::write(root.join("macros/probe_positive.sql"), TEST_MACRO).unwrap();
    let (mut lsp, _uri) = project.booted();
    let uri = cogitator_lsp::fs_path_to_uri(&root.join("models/probe_props.yml"));
    lsp.notify(
        "textDocument/didOpen",
        serde_json::json!({"textDocument": {"uri": uri, "languageId": "yaml", "version": 1,
            "text": YAML}}),
    );

    // config keys from the schema, at the start of `materialized:`
    let config = labels_at(&mut lsp, &uri, 5, 6);
    assert!(config.iter().any(|l| l == "materialized"), "{config:?}");
    assert!(config.iter().any(|l| l == "contract"), "{config:?}");
    // a `name:` under columns: the model's output columns
    let columns = labels_at(&mut lsp, &uri, 7, 14);
    assert!(columns.iter().any(|l| l == "last_name"), "{columns:?}");
    // a test item: built-ins and the project's own generic test
    let tests = labels_at(&mut lsp, &uri, 9, 12);
    for test in ["not_null", "accepted_values", "relationships", "positive"] {
        assert!(tests.iter().any(|l| l == test), "{test} missing: {tests:?}");
    }

    // the tested column the model never produces
    let pulled = lsp.request(
        "textDocument/diagnostic",
        serde_json::json!({"textDocument": {"uri": uri}}),
    );
    let items = pulled["result"]["items"].as_array().expect("items").clone();
    let diag = items
        .iter()
        .find(|d| d["code"] == "yaml-unproduced-column")
        .unwrap_or_else(|| panic!("no unproduced-column warning in {items:?}"))
        .clone();
    assert_eq!(diag["severity"], 2, "{diag}");
    assert_eq!(diag["range"]["start"]["line"], 10, "{diag}");
    let actions = lsp.request(
        "textDocument/codeAction",
        serde_json::json!({"textDocument": {"uri": uri}, "range": diag["range"],
                           "context": {"diagnostics": [diag], "only": ["quickfix"]}}),
    );
    let list = actions["result"].as_array().expect("actions").clone();
    let rename = list
        .iter()
        .find(|a| a["title"] == "Rename column 'frist_name' to 'first_name'")
        .unwrap_or_else(|| panic!("no rename in {list:?}"));
    assert_eq!(
        rename["edit"]["documentChanges"][0]["edits"][0]["newText"],
        "first_name"
    );
    assert!(list
        .iter()
        .any(|a| a["title"] == "Remove column 'frist_name' from 'probe_props'"));

    // "Add missing columns" from the model's own YAML entry
    let actions = lsp.request(
        "textDocument/codeAction",
        serde_json::json!({"textDocument": {"uri": uri},
                           "range": {"start": {"line": 3, "character": 4},
                                     "end": {"line": 3, "character": 4}},
                           "context": {"diagnostics": []}}),
    );
    let list = actions["result"].as_array().expect("actions").clone();
    let add = list
        .iter()
        .find(|a| {
            a["title"].as_str().is_some_and(|t| {
                t.starts_with("Add") && t.contains("missing column(s) of 'probe_props'")
            })
        })
        .unwrap_or_else(|| panic!("no add-columns action in {list:?}"));
    let new_text = add["edit"]["documentChanges"][0]["edits"][0]["newText"]
        .as_str()
        .unwrap_or_default();
    assert!(new_text.contains("- name: first_name"), "{add}");
    assert!(new_text.contains("- name: last_name"), "{add}");

    lsp.shutdown();
}
