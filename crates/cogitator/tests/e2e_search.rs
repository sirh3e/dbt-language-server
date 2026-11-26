//! End-to-end suite (specs/rust-port/05 §7) — this file's theme only;
//! the framed-LSP harness lives in tests/harness/.
//!
//! dbt.search: a structural query over the models' HIR, answered in
//! dbt.astGrep's match shape, with the models it could not search.

mod harness;
use harness::jaffle;
use serde_json::json;

#[test]
fn a_structural_query_over_the_project() {
    let project = jaffle();
    let (mut lsp, _) = project.booted();
    lsp.wait_refresh(1);
    let answer = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.search",
               "arguments": [{"uri": "", "query": "reads stg_customers and select-star"}]}),
    );
    let result = &answer["result"];
    assert!(
        result["models"]
            .as_array()
            .is_some_and(|m| m.iter().any(|m| m == "customers")),
        "{answer}"
    );
    let first = &result["matches"][0];
    assert!(first["uri"].as_str().unwrap().ends_with(".sql"), "{first}");
    assert!(first["range"]["start"]["line"].is_u64(), "{first}");
    assert!(first["env"]["clause"].is_string(), "{first}");
    assert!(first["replacement"].is_null(), "{first}");
    assert!(
        result["notSearched"]
            .as_array()
            .is_some_and(|n| n.iter().any(|n| n["model"] == "orders")),
        "control blocks are reported, not dropped: {answer}"
    );

    // a query it cannot read is InvalidParams with guidance
    let bad = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.search", "arguments": [{"uri": "", "query": "find it"}]}),
    );
    assert!(
        bad["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("cannot read the query")),
        "{bad}"
    );
}

#[test]
fn or_and_not_over_the_project() {
    let project = jaffle();
    let (mut lsp, _) = project.booted();
    lsp.wait_refresh(1);
    let answer = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.search",
               "arguments": [{"uri": "",
                              "query": "(reads raw_customers or reads raw_orders) and not reads raw_orders"}]}),
    );
    assert_eq!(
        answer["result"]["models"],
        json!(["stg_customers"]),
        "{answer}"
    );
    let clauses: Vec<&str> = answer["result"]["matches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["env"]["clause"].as_str().unwrap())
        .collect();
    assert_eq!(clauses, ["reads raw_customers"], "{answer}");

    let bad = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.search", "arguments": [{"uri": "", "query": "(reads a"}]}),
    );
    assert!(
        bad["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("not closed")),
        "{bad}"
    );
}
