//! End-to-end suite (specs/rust-port/05 §7) — this file's theme only;
//! the framed-LSP harness lives in tests/harness/.
//!
//! The column reach: dbt.impact with a column names the semantic
//! models, tests and BI dashboards that read it, and the hover on the
//! column says so once the open/save job has cached it.

mod harness;
use harness::jaffle;
use serde_json::json;

const SEMANTIC: &str = "version: 2
semantic_models:
  - name: customers_sm
    model: ref('customers')
    entities:
      - name: customer_id
        type: primary
    measures:
      - name: customer_count
        expr: customer_id
        agg: count_distinct
metrics:
  - name: active_customers
    type: simple
    type_params:
      measure: customer_count
";

const BI: &str = r#"{"dashboards": [
  {"name": "Customer board", "url": "https://bi.example/customers",
   "reads": [{"model": "customers", "column": "customer_id"}]},
  {"name": "Unrelated", "reads": [{"model": "customers", "column": "first_name"}]}
]}"#;

#[test]
fn a_column_reaches_semantic_models_metrics_and_dashboards() {
    let project = jaffle();
    let root = project.root.clone();
    std::fs::write(root.join("models/semantic.yml"), SEMANTIC).unwrap();
    std::fs::write(root.join("bi.json"), BI).unwrap();
    std::fs::write(root.join("cogitator.yml"), "bi:\n  manifest: bi.json\n").unwrap();
    let (mut lsp, _) = project.booted();

    let reach = lsp.wait_until("the column reach names the dashboard", |lsp| {
        let impact = lsp.request(
            "workspace/executeCommand",
            json!({"command": "dbt.impact",
                   "arguments": [{"model": "stg_customers", "column": "customer_id"}]}),
        );
        let reach = impact["result"]["reach"].clone();
        let dashboards = reach["dashboards"].as_array().map_or(0, Vec::len);
        let semantic = reach["semanticItems"].as_array().map_or(0, Vec::len);
        (dashboards == 1 && semantic >= 1).then_some(impact["result"].clone())
    });
    let r = &reach["reach"];
    assert_eq!(r["dashboards"][0]["name"], "Customer board", "{reach}");
    assert_eq!(r["dashboards"][0]["level"], "column");
    assert!(
        r["semanticItems"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["semanticModel"] == "customers_sm" && s["list"] == "entity"),
        "{reach}"
    );
    assert_eq!(r["metrics"][0]["name"], "active_customers", "{reach}");
    assert!(
        r["tests"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["model"] == "customers" && t["test"] == "unique"),
        "{reach}"
    );
    assert!(
        reach["summary"]
            .as_str()
            .is_some_and(|s| s.contains("read by") && s.contains("1 dashboard")),
        "{reach}"
    );

    // the hover reads the reach the open job cached
    let model = root.join("models/staging/stg_customers.sql");
    let text = std::fs::read_to_string(&model).unwrap();
    let uri = cogitator_lsp::fs_path_to_uri(&model);
    lsp.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": uri, "languageId": "sql", "version": 1, "text": text}}),
    );
    let (line, character) = text
        .lines()
        .enumerate()
        .find_map(|(i, l)| l.find("as customer_id").map(|c| (i, c + 3)))
        .expect("the alias");
    let hover = lsp.wait_until("the hover names the reach", |lsp| {
        let hover = lsp.request(
            "textDocument/hover",
            json!({"textDocument": {"uri": uri},
                   "position": {"line": line, "character": character}}),
        );
        let value = hover["result"]["contents"]["value"]
            .as_str()
            .or_else(|| hover["result"]["contents"].as_str())
            .unwrap_or_default()
            .to_string();
        value.contains("is read by").then_some(value)
    });
    assert!(
        hover.contains("`stg_customers.customer_id` is read by"),
        "{hover}"
    );
    assert!(hover.contains("1 dashboard"), "{hover}");
}
