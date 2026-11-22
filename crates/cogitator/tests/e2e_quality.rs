//! End-to-end suite (specs/rust-port/05 §7) — this file's theme only:
//! the quality lenses (coverage, "Run this test") and the unit-test
//! scaffold.

mod harness;
use harness::jaffle;

fn titles(result: &serde_json::Value) -> Vec<String> {
    result
        .as_array()
        .expect("lenses")
        .iter()
        .filter_map(|l| l["command"]["title"].as_str().map(str::to_string))
        .collect()
}

/// Coverage on the SQL side and per column in the YAML, the project
/// summary in dbt.status, and the scaffold action on an open model.
#[test]
fn coverage_lenses_status_and_unit_test_scaffold() {
    let project = jaffle();
    let root = project.root.clone();
    let (mut lsp, customers_uri) = project.booted();

    let lenses = lsp.request(
        "textDocument/codeLens",
        serde_json::json!({"textDocument": {"uri": customers_uri}}),
    );
    let sql_titles = titles(&lenses["result"]);
    assert!(
        sql_titles
            .iter()
            .any(|t| t.contains("columns documented") && t.contains("tested")),
        "{sql_titles:?}"
    );

    let staging = cogitator_lsp::fs_path_to_uri(&root.join("models/staging/schema.yml"));
    let lenses = lsp.request(
        "textDocument/codeLens",
        serde_json::json!({"textDocument": {"uri": staging}}),
    );
    let yaml_titles = titles(&lenses["result"]);
    assert!(
        yaml_titles.contains(&"0/1 columns documented · 1/1 tested · no description".to_string()),
        "{yaml_titles:?}"
    );
    assert!(
        yaml_titles.contains(&"no description".to_string()),
        "{yaml_titles:?}"
    );

    let status = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({ "command": "dbt.status", "arguments": [{}] }),
    );
    let coverage = &status["result"]["project"]["coverage"];
    assert!(coverage["models"].as_u64().unwrap_or(0) >= 3, "{coverage}");
    assert!(coverage["tested"].as_u64().unwrap_or(0) >= 1, "{coverage}");

    let orders = root.join("models/orders.sql");
    let orders_uri = cogitator_lsp::fs_path_to_uri(&orders);
    lsp.notify(
        "textDocument/didOpen",
        serde_json::json!({"textDocument": {
            "uri": orders_uri, "languageId": "sql", "version": 1,
            "text": std::fs::read_to_string(&orders).unwrap()}}),
    );
    let actions = lsp.request(
        "textDocument/codeAction",
        serde_json::json!({"textDocument": {"uri": orders_uri},
                           "range": {"start": {"line": 0, "character": 0},
                                     "end": {"line": 0, "character": 0}},
                           "context": {"diagnostics": []}}),
    );
    let scaffold = actions["result"]
        .as_array()
        .expect("actions")
        .iter()
        .find(|a| a["title"] == "Scaffold a unit test for 'orders'")
        .unwrap_or_else(|| panic!("no scaffold action: {actions}"));
    let edit = &scaffold["edit"]["documentChanges"][0];
    assert!(
        edit["textDocument"]["uri"]
            .as_str()
            .is_some_and(|u| u.ends_with("schema.yml")),
        "{edit}"
    );
    let text = edit["edits"][0]["newText"].as_str().unwrap();
    assert!(text.contains("- input: ref('stg_orders')"), "{text}");
    assert!(text.contains("- input: ref('stg_payments')"), "{text}");
    lsp.shutdown();
}

/// "Run this test" on a generic YAML test runs exactly that test.
#[cfg(feature = "e2e")]
#[test]
fn run_this_test_lens_selects_the_one_test() {
    let project = jaffle();
    let root = project.root.clone();
    let fake = harness::fake::install(harness::fake::FakeDbt::printing("done\n"));
    let envs = fake.envs();
    let env_refs: Vec<(&str, &str)> = envs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let (mut lsp, _uri) = project.booted_with(&env_refs, serde_json::json!({}));

    let schema = cogitator_lsp::fs_path_to_uri(&root.join("models/schema.yml"));
    let lenses = lsp.request(
        "textDocument/codeLens",
        serde_json::json!({"textDocument": {"uri": schema}}),
    );
    let lens = lenses["result"]
        .as_array()
        .expect("lenses")
        .iter()
        .find(|l| l["command"]["arguments"][0]["selector"] == "not_null_orders_order_id")
        .unwrap_or_else(|| panic!("no lens for the test: {lenses}"))
        .clone();
    assert_eq!(lens["command"]["title"], "Run this test");

    let started = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({"command": lens["command"]["command"],
                           "arguments": lens["command"]["arguments"]}),
    );
    assert!(started["result"]["token"].as_u64().is_some(), "{started}");
    lsp.wait_notification("window/showMessage", |frame| {
        frame["params"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("dbt test succeeded"))
    });
    let args = fake.logged_args();
    assert!(
        args.iter()
            .any(|line| line.contains("test") && line.contains("not_null_orders_order_id")),
        "the fake dbt saw the one test: {args:?}"
    );
    lsp.shutdown();
}
