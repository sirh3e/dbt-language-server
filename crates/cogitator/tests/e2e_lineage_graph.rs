//! `dbt.columnLineageGraph` over the wire: the walk in one answer, served
//! from the project-wide cache once computed, and from disk after a
//! restart.

mod harness;
use harness::jaffle;
use harness::Lsp;
use serde_json::{json, Value};

fn graph(lsp: &mut Lsp, uri: &str, column: &str, direction: &str) -> Value {
    lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.columnLineageGraph",
               "arguments": [{"uri": uri, "column": column, "direction": direction, "depth": 3}]}),
    )
}

/// Waits for the schema job: lineage keys on the upstream schemas, so a
/// "cached" answer is only stable once they are published.
fn schemas_published(lsp: &mut Lsp, uri: &str) {
    lsp.wait_until("the schema job publishes a revision", |lsp| {
        let plan = lsp.request(
            "workspace/executeCommand",
            json!({"command": "dbt.queryPlan", "arguments": [{"uri": uri}]}),
        );
        plan["result"]["schemaRevision"]
            .as_u64()
            .is_some_and(|r| r > 0)
            .then_some(())
    });
}

fn ids(answer: &Value) -> Vec<String> {
    answer["result"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_str().unwrap().to_string())
        .collect()
}

fn cache_line(lsp: &mut Lsp) -> Value {
    lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.cache", "arguments": [{}]}),
    )["result"]["lineage_graph"]
        .clone()
}

#[test]
fn the_graph_is_walked_once_cached_and_kept_across_a_restart() {
    let project = jaffle();
    let (mut lsp, uri) = project.booted();
    schemas_published(&mut lsp, &uri);

    let first = graph(&mut lsp, &uri, "customer_id", "both");
    let result = &first["result"];
    assert_eq!(result["start"], "model:customers::customer_id", "{first}");
    let nodes = ids(&first);
    assert!(
        nodes.contains(&"model:stg_customers::customer_id".to_string()),
        "{first}"
    );
    let start = result["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == "model:customers::customer_id")
        .unwrap();
    assert_eq!(start["kind"], "model");
    assert_eq!(start["model"], "customers");
    assert_eq!(start["column"], "customer_id");
    assert_eq!(start["tags"], json!([]));
    assert!(start["uri"].as_str().unwrap().ends_with("customers.sql"));
    let into_start: Vec<&Value> = result["edges"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["to"] == "model:customers::customer_id")
        .collect();
    assert!(!into_start.is_empty(), "{first}");
    assert!(into_start
        .iter()
        .all(|e| e["kind"] == "direct" || e["kind"] == "derived"));

    // up only: nothing downstream of the start
    let up = graph(&mut lsp, &uri, "customer_id", "up");
    assert!(up["result"]["edges"]
        .as_array()
        .unwrap()
        .iter()
        .all(|e| e["from"] != "model:customers::customer_id"));

    // the whole project is computed behind the request, then written out
    let cached = lsp.wait_until("the cache is filled", |lsp| {
        let line = cache_line(lsp);
        let answer = graph(lsp, &uri, "customer_id", "both");
        (line["pending"] == 0 && answer["result"]["cached"] == true).then_some(answer)
    });
    assert_eq!(ids(&cached), nodes, "the same graph from the cache");
    let file = project.root.join("target/.cogitator-lineage-cache");
    lsp.wait_until("the cache is on disk", |_| file.is_file().then_some(()));
    let line = cache_line(&mut lsp);
    assert!(line["entries"].as_u64().is_some_and(|e| e >= 5), "{line}");
    let status = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.status", "arguments": [{}]}),
    );
    assert!(
        status["result"]["caches"]["lineage_graph"]["entries"].is_u64(),
        "{status}"
    );
    lsp.shutdown();

    // a new process reads the file and computes nothing
    let (mut lsp, uri) = project.booted();
    schemas_published(&mut lsp, &uri);
    let again = graph(&mut lsp, &uri, "customer_id", "both");
    assert_eq!(again["result"]["cached"], true, "{again}");
    assert_eq!(ids(&again), nodes);
    let line = cache_line(&mut lsp);
    assert!(line["loaded"].as_u64().is_some_and(|l| l >= 5), "{line}");
}

#[test]
fn a_bad_direction_is_refused_with_its_fix() {
    let project = jaffle();
    let (mut lsp, uri) = project.booted();
    let answer = graph(&mut lsp, &uri, "customer_id", "sideways");
    let message = answer["error"]["message"].as_str().unwrap_or_default();
    assert!(message.contains("\"up\""), "{answer}");
}
