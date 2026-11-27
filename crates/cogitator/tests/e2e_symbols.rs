//! End-to-end suite (specs/rust-port/05 §7) — finding things by name:
//! workspace/symbol's plain query (models, source tables, columns,
//! tests, with their containers) and its `column:`/`test:` prefixes, and
//! documentSymbol's ref() calls beside the CTEs.

mod harness;
use harness::jaffle;
use serde_json::json;

fn named<'a>(response: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    response["result"]
        .as_array()
        .and_then(|a| a.iter().find(|s| s["name"] == name))
        .unwrap_or_else(|| panic!("no {name} in {response}"))
}

#[test]
fn workspace_symbol_finds_parts_of_nodes() {
    let project = jaffle();
    let (mut lsp, _) = project.booted();

    let found = lsp.request("workspace/symbol", json!({"query": "customer_id"}));
    let column = named(&found, "customers.customer_id");
    assert_eq!(column["kind"], 8, "{column}");
    assert_eq!(column["containerName"], "customers", "{column}");
    assert!(column["location"]["uri"]
        .as_str()
        .unwrap()
        .ends_with("models/schema.yml"));
    let test = named(&found, "unique on customers.customer_id");
    assert_eq!(test["kind"], 6, "{test}");
    assert_eq!(test["containerName"], "customers", "{test}");

    let tables = lsp.request("workspace/symbol", json!({"query": "stripe"}));
    let table = named(&tables, "stripe.payments");
    assert_eq!(
        (table["kind"].clone(), table["containerName"].clone()),
        (json!(11), json!("stripe"))
    );
    // a model carries no container: the field is left out
    let model = named(&tables, "stg_stripe_payments");
    assert!(model.get("containerName").is_none(), "{model}");

    let columns = lsp.request("workspace/symbol", json!({"query": "column:order_id"}));
    assert!(
        columns["result"]
            .as_array()
            .is_some_and(|a| !a.is_empty() && a.iter().all(|s| s["kind"] == 8)),
        "{columns}"
    );
    let tests = lsp.request("workspace/symbol", json!({"query": "test:not_null"}));
    assert!(
        tests["result"]
            .as_array()
            .is_some_and(|a| !a.is_empty() && a.iter().all(|s| s["kind"] == 6)),
        "{tests}"
    );
}

#[test]
fn document_symbol_lists_refs_and_ctes() {
    let project = jaffle();
    let (mut lsp, customers_uri) = project.booted();
    let symbols = lsp.request(
        "textDocument/documentSymbol",
        json!({"textDocument": {"uri": customers_uri}}),
    );
    let listed: Vec<(String, i64)> = symbols["result"]
        .as_array()
        .expect("symbols")
        .iter()
        .take(4)
        .map(|s| {
            (
                s["name"].as_str().unwrap().to_string(),
                s["kind"].as_i64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        listed,
        vec![
            ("customers".to_string(), 23),
            ("ref(stg_customers)".to_string(), 4),
            ("orders".to_string(), 23),
            ("ref(stg_orders)".to_string(), 4),
        ],
        "{symbols}"
    );
}
