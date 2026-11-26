//! dbt.testSql and dbt.incrementalSql, and the code actions that open
//! them and the strategy templates, end to end on copies of the jaffle
//! fixture.

mod harness;
use harness::{jaffle, Lsp};
use serde_json::{json, Value};

fn open(lsp: &mut Lsp, uri: &str, text: &str) {
    lsp.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": uri, "languageId": "sql", "version": 1, "text": text}}),
    );
}

fn actions(lsp: &mut Lsp, uri: &str) -> Vec<Value> {
    let reply = lsp.request(
        "textDocument/codeAction",
        json!({"textDocument": {"uri": uri},
               "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}},
               "context": {"diagnostics": []}}),
    );
    reply["result"].as_array().cloned().unwrap_or_default()
}

#[test]
fn the_tests_of_orders_render_as_dbt_runs_them() {
    let project = jaffle();
    let path = project.root.join("models/orders.sql");
    let text = std::fs::read_to_string(&path).unwrap();
    let uri = cogitator_lsp::fs_path_to_uri(&path);
    let (mut lsp, _) = project.booted();
    open(&mut lsp, &uri, &text);

    let reply = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.testSql", "arguments": [{"uri": uri}]}),
    );
    let result = &reply["result"];
    assert_eq!(result["model"], "orders", "{reply}");
    let tests = result["tests"].as_array().expect("tests");
    let label = |t: &Value| t["label"].as_str().unwrap_or_default().to_string();
    let labels: Vec<String> = tests.iter().map(label).collect();
    for want in [
        "unique on order_id",
        "not_null on order_id",
        "relationships on customer_id",
        "accepted_values on status",
        "singular test assert_positive_order_amount",
    ] {
        assert!(
            labels.iter().any(|l| l == want),
            "{want} missing from {labels:?}"
        );
    }
    let relationships = tests
        .iter()
        .find(|t| label(t) == "relationships on customer_id")
        .unwrap();
    let sql = relationships["sql"].as_str().expect("rendered");
    assert!(sql.contains("from ") && sql.contains("customers"), "{sql}");
    let query = relationships["query"].as_str().unwrap();
    assert!(
        query.starts_with("select\n    count(*) as failures,"),
        "{query}"
    );
    assert!(tests.iter().all(|t| t["error"].is_null()), "{reply}");

    // the code action opens them in every client
    let found = actions(&mut lsp, &uri);
    let show = found
        .iter()
        .find(|a| a["command"]["command"] == "dbt.testSql")
        .unwrap_or_else(|| panic!("a test SQL action in {found:?}"));
    assert!(
        show["title"]
            .as_str()
            .unwrap()
            .starts_with("Show the SQL of "),
        "{show}"
    );
    let _ = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.testSql", "arguments": [show["command"]["arguments"][0]]}),
    );
    let shown = lsp.wait_server_request("window/showDocument", |m| {
        m["params"]["uri"]
            .as_str()
            .is_some_and(|u| u.ends_with("tests/orders.sql"))
    });
    let path = cogitator_lsp::uri_to_fs_path(shown["params"]["uri"].as_str().unwrap()).unwrap();
    let written = std::fs::read_to_string(path).unwrap();
    assert!(
        written.contains("-- ── accepted_values on status"),
        "{written}"
    );
}

#[test]
fn a_table_is_offered_the_strategy_templates_and_an_incremental_model_its_sql() {
    let project = jaffle();
    let path = project.root.join("models/orders.sql");
    let text = std::fs::read_to_string(&path).unwrap();
    let uri = cogitator_lsp::fs_path_to_uri(&path);
    let (mut lsp, _) = project.booted();
    open(&mut lsp, &uri, &text);
    let titles: Vec<String> = actions(&mut lsp, &uri)
        .iter()
        .filter(|a| a["kind"] == "refactor.rewrite")
        .filter_map(|a| a["title"].as_str().map(str::to_string))
        .collect();
    assert!(
        titles
            .iter()
            .any(|t| t == "Make incremental with the merge strategy (unique_key='order_id')"),
        "{titles:?}"
    );
    assert_eq!(
        titles
            .iter()
            .filter(|t| t.starts_with("Make incremental"))
            .count(),
        5,
        "{titles:?}"
    );

    let incr = project.root.join("models/orders_incr.sql");
    let body = "{{ config(materialized='incremental', unique_key='order_id') }}\n\
                select order_id, status from {{ ref('stg_orders') }}\n\
                {% if is_incremental() %}\nwhere order_id > (select max(order_id) from {{ this }})\n{% endif %}\n";
    std::fs::write(&incr, body).unwrap();
    let incr_uri = cogitator_lsp::fs_path_to_uri(&incr);
    open(&mut lsp, &incr_uri, body);
    let reply = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.incrementalSql", "arguments": [{"uri": incr_uri, "strategy": "merge"}]}),
    );
    let result = &reply["result"];
    assert_eq!(result["strategy"], "merge", "{reply}");
    assert_eq!(result["strategySource"], "argument", "{reply}");
    let text = result["text"].as_str().unwrap();
    assert!(
        text.contains("merge into ") && text.contains("on DBT_INTERNAL_SOURCE.order_id"),
        "{text}"
    );
    let reply = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.incrementalSql", "arguments": [{"uri": incr_uri, "strategy": "nope"}]}),
    );
    assert!(
        reply["error"]["message"]
            .as_str()
            .unwrap()
            .contains("not an incremental strategy"),
        "{reply}"
    );
}
