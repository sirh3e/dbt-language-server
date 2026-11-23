//! Output-column types end to end on a jaffle copy: `dbt.modelInfo`
//! carries them, a contracted `data_type:` the SQL cannot produce is an
//! error with its fix, the YAML hover shows the inferred type, and the
//! fill action writes the missing types.

mod harness;
use harness::jaffle;
use serde_json::{json, Value};

const SQL: &str = "select\n    cast(order_id as bigint) as order_id,\n    count(*) as order_count,\n    cast(order_date as date) as first_day\nfrom {{ ref('stg_orders') }}\ngroup by 1, 3\n";

const YAML: &str = "version: 2\nmodels:\n  - name: typed_orders\n    config:\n      contract:\n        enforced: true\n    columns:\n      - name: order_id\n        data_type: varchar\n      - name: order_count\n        data_type: bigint\n      - name: first_day\n";

fn model_info(lsp: &mut harness::Lsp, uri: &str) -> Value {
    lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.modelInfo", "arguments": [{"uri": uri}]}),
    )["result"]
        .clone()
}

#[test]
fn column_types_reach_model_info_diagnostics_hover_and_fixes() {
    let project = jaffle();
    let sql = project.root.join("models/typed_orders.sql");
    let yml = project.root.join("models/typed_orders.yml");
    std::fs::write(&sql, SQL).expect("model");
    std::fs::write(&yml, YAML).expect("properties");
    let (mut lsp, _uri) = project.booted();
    let sql_uri = cogitator_lsp::fs_path_to_uri(&sql);
    let yml_uri = cogitator_lsp::fs_path_to_uri(&yml);

    // the schema job runs after the refresh; modelInfo shows its types
    let info = lsp.wait_until("the schema job types typed_orders", |lsp| {
        let info = model_info(lsp, &sql_uri);
        info["columnTypes"]["columns"].is_array().then_some(info)
    });
    let columns = info["columnTypes"]["columns"].as_array().expect("columns");
    let order_id = columns
        .iter()
        .find(|c| c["name"] == "order_id")
        .expect("order_id");
    assert_eq!(order_id["dataType"], "bigint", "{info}");
    assert_eq!(order_id["basis"], "proven", "{info}");
    assert_eq!(order_id["provenance"]["kind"], "cast", "{info}");

    lsp.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": yml_uri, "languageId": "yaml",
            "version": 1, "text": YAML}}),
    );
    let pulled = lsp.request(
        "textDocument/diagnostic",
        json!({"textDocument": {"uri": yml_uri}}),
    );
    let items = pulled["result"]["items"].as_array().expect("items");
    let mismatches: Vec<&Value> = items
        .iter()
        .filter(|d| d["code"] == "column-type-mismatch")
        .collect();
    assert_eq!(mismatches.len(), 1, "{items:?}");
    let mismatch = mismatches[0];
    assert_eq!(mismatch["severity"], 1, "{mismatch}");
    assert_eq!(mismatch["range"]["start"]["line"], 8, "{mismatch}");
    assert!(
        items.iter().all(|d| d["code"] != "contract-drift"),
        "{items:?}"
    );

    // the quickfix from the diagnostic's data
    let actions = lsp.request(
        "textDocument/codeAction",
        json!({"textDocument": {"uri": yml_uri},
               "range": mismatch["range"],
               "context": {"diagnostics": [mismatch]}}),
    );
    let titles: Vec<&str> = actions["result"]
        .as_array()
        .expect("actions")
        .iter()
        .filter_map(|a| a["title"].as_str())
        .collect();
    assert!(titles.contains(&"Set data_type to bigint"), "{titles:?}");
    assert!(
        titles
            .iter()
            .any(|t| t.starts_with("Fill data_type from the inferred types (1 column")),
        "{titles:?}"
    );

    let hover = lsp.request(
        "textDocument/hover",
        json!({"textDocument": {"uri": yml_uri},
               "position": {"line": 7, "character": 16}}),
    );
    let text = hover["result"]["contents"]["value"]
        .as_str()
        .or_else(|| hover["result"]["contents"].as_str())
        .unwrap_or_default()
        .to_string();
    assert!(text.contains("Type: `bigint`"), "{hover}");
    assert!(text.contains("Declared `varchar`"), "{hover}");

    // in the model's SQL: the output column's type and why
    lsp.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": sql_uri, "languageId": "sql",
            "version": 1, "text": SQL}}),
    );
    let hover = lsp.request(
        "textDocument/hover",
        json!({"textDocument": {"uri": sql_uri},
               "position": {"line": 2, "character": 18}}),
    );
    let text = hover["result"]["contents"]["value"]
        .as_str()
        .or_else(|| hover["result"]["contents"].as_str())
        .unwrap_or_default();
    assert!(text.contains("Output column `order_count`"), "{hover}");
    assert!(text.contains("aggregate count()"), "{hover}");

    lsp.shutdown();
}
