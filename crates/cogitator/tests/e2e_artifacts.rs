//! End-to-end suite (specs/rust-port/05 §7) — this file's theme only;
//! the framed-LSP harness lives in tests/harness/.

mod harness;
#[allow(unused_imports)]
use harness::*;

/// dbt artifacts end to end: fake target/ JSONs feed the last-run
/// lens, run/test-failure diagnostics, the drift hint, and the stamp
/// check picks up a rewrite without restart (03 §10).
#[test]
fn artifact_backed_lenses_and_diagnostics() {
    let src = fixture_root().canonicalize().unwrap();
    let root = std::env::temp_dir().join(format!("cogitator-e2e-artifacts-{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    copy_fixture(&src, &root);
    let _cleanup = RemoveOnDrop(root.clone());
    let target = root.join("target");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(
        target.join("run_results.json"),
        r#"{"metadata": {"generated_at": "2026-11-03T10:00:00Z"},
            "results": [
              {"unique_id": "model.jaffle_shop.customers", "status": "success", "execution_time": 3.2},
              {"unique_id": "model.jaffle_shop.orders", "status": "error", "message": "boom"},
              {"unique_id": "test.jaffle_shop.not_null_customers_customer_id.abc",
               "status": "fail", "message": "Got 3 results"}
            ]}"#,
    )
    .unwrap();
    std::fs::write(
        target.join("manifest.json"),
        r#"{"nodes": {
            "model.jaffle_shop.customers": {"resource_type": "model", "name": "customers"},
            "model.jaffle_shop.orders": {"resource_type": "model", "name": "orders"},
            "test.jaffle_shop.not_null_customers_customer_id.abc":
                {"resource_type": "test", "attached_node": "model.jaffle_shop.customers"}
        }}"#,
    )
    .unwrap();

    let (mut lsp, customers_uri) = booted_in(&root, &[]);

    // 1. the last-run lens on a green model
    let lenses = lsp.request(
        "textDocument/codeLens",
        serde_json::json!({"textDocument": {"uri": customers_uri}}),
    );
    let titles: Vec<&str> = lenses["result"]
        .as_array()
        .expect("lenses")
        .iter()
        .filter_map(|l| l["command"]["title"].as_str())
        .collect();
    assert!(
        titles.iter().any(|t| t.starts_with("last run: OK in 3.2")),
        "{titles:?}"
    );

    // 2. failed run → warning with the adapter message
    let orders_uri = cogitator_lsp::fs_path_to_uri(&root.join("models/orders.sql"));
    let pulled = lsp.request(
        "textDocument/diagnostic",
        serde_json::json!({"textDocument": {"uri": orders_uri}}),
    );
    let items = pulled["result"]["items"].as_array().expect("items");
    assert!(
        items.iter().any(|d| d["code"] == "artifact-run-failed"
            && d["message"].as_str().is_some_and(|m| m.contains("boom"))),
        "{items:?}"
    );

    // 3. failed test lands on the model file AND the schema entry
    let pulled = lsp.request(
        "textDocument/diagnostic",
        serde_json::json!({"textDocument": {"uri": customers_uri}}),
    );
    assert!(
        pulled["result"]["items"]
            .as_array()
            .is_some_and(|i| i.iter().any(|d| d["code"] == "artifact-test-failed")),
        "{pulled}"
    );
    let schema_uri = cogitator_lsp::fs_path_to_uri(&root.join("models/schema.yml"));
    let pulled = lsp.request(
        "textDocument/diagnostic",
        serde_json::json!({"textDocument": {"uri": schema_uri}}),
    );
    let schema_item = pulled["result"]["items"]
        .as_array()
        .and_then(|i| {
            i.iter()
                .find(|d| d["code"] == "artifact-test-failed")
                .cloned()
        })
        .expect("schema test-failure diagnostic");
    assert!(
        schema_item["range"]["start"]["line"].as_u64().unwrap() > 0,
        "anchored at the model's entry: {schema_item}"
    );

    // 4. a model the manifest misses gets the drift hint
    let drift_uri = cogitator_lsp::fs_path_to_uri(&root.join("models/customer_orders.sql"));
    let pulled = lsp.request(
        "textDocument/diagnostic",
        serde_json::json!({"textDocument": {"uri": drift_uri}}),
    );
    let drift = pulled["result"]["items"]
        .as_array()
        .and_then(|i| i.iter().find(|d| d["code"] == "artifact-drift").cloned())
        .expect("drift hint");
    assert!(
        drift["relatedInformation"][0]["location"]["uri"]
            .as_str()
            .is_some_and(|u| u.ends_with("manifest.json")),
        "{drift}"
    );

    // 4b. freshness verdicts become lenses on the source YAML entries
    std::fs::write(
        target.join("sources.json"),
        r#"{"results": [{"unique_id": "source.jaffle_shop.stripe.payments",
                         "status": "pass", "snapshotted_at": "2026-11-03T09:00:00Z"}]}"#,
    )
    .unwrap();
    let mut fresh_titles: Vec<String> = Vec::new();
    for _ in 0..100 {
        let lenses = lsp.request(
            "textDocument/codeLens",
            serde_json::json!({"textDocument": {"uri": schema_uri}}),
        );
        fresh_titles = lenses["result"]
            .as_array()
            .map(|l| {
                l.iter()
                    .filter_map(|x| x["command"]["title"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        if fresh_titles
            .iter()
            .any(|t| t.starts_with("freshness: pass"))
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(
        fresh_titles
            .iter()
            .any(|t| t == "freshness: pass · 2026-11-03T09:00:00Z"),
        "{fresh_titles:?}"
    );

    // 5. the stamp check sees a rewrite — no restart, no reload command
    std::fs::write(
        target.join("run_results.json"),
        r#"{"results": [{"unique_id": "model.jaffle_shop.customers", "status": "error",
                         "message": "late failure"}]}"#,
    )
    .unwrap();
    let mut changed = false;
    for _ in 0..100 {
        let lenses = lsp.request(
            "textDocument/codeLens",
            serde_json::json!({"textDocument": {"uri": customers_uri}}),
        );
        if lenses["result"]
            .as_array()
            .is_some_and(|l| l.iter().any(|x| x["command"]["title"] == "last run: ERROR"))
        {
            changed = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(changed, "stamp refresh never surfaced the rewrite");

    let _ = lsp.request("shutdown", serde_json::json!({}));
    lsp.notify("exit", serde_json::json!({}));
}

/// dbt.doctor (05 §9): the merged health report over a project with
/// synthesized artifacts.
#[test]
fn doctor_reports_the_whole_picture() {
    let src = fixture_root().canonicalize().unwrap();
    let root = std::env::temp_dir().join(format!("cogitator-e2e-doctor-{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    copy_fixture(&src, &root);
    let _cleanup = RemoveOnDrop(root.clone());
    let target = root.join("target");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(
        target.join("run_results.json"),
        r#"{"results": [{"unique_id": "model.jaffle_shop.customers", "status": "success"}]}"#,
    )
    .unwrap();
    std::fs::write(root.join("cogitator.yml"), "lint:\n  select_star: true\n").unwrap();
    std::fs::write(
        target.join("catalog.json"),
        r#"{"nodes": {}, "sources": {}}"#,
    )
    .unwrap();

    let (mut lsp, _uri) = booted_in(&root, &[]);
    let mut report = serde_json::Value::Null;
    for _ in 0..100 {
        let answer = lsp.request(
            "workspace/executeCommand",
            serde_json::json!({"command": "dbt.doctor", "arguments": [{}]}),
        );
        report = answer["result"].clone();
        // wait until the hot config applied the lint flag
        if report["lintEnabled"]
            .as_array()
            .is_some_and(|l| l.iter().any(|x| x == "select_star"))
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(report["artifacts"]["catalogAgeS"].is_u64(), "{report}");
    assert_eq!(report["projectName"], "jaffle_shop", "{report}");
    assert_eq!(report["dialect"], "duckdb", "{report}");
    assert!(report["graph"]["nodes"].as_u64().unwrap() > 5, "{report}");
    assert!(report["artifacts"]["runResultsAgeS"].is_u64(), "{report}");
    assert!(report["dbtBinary"].is_string(), "{report}");
    assert!(report["config"]["loaded"].as_bool().unwrap(), "{report}");
    assert!(
        report["lintEnabled"]
            .as_array()
            .is_some_and(|l| l.len() == 1),
        "{report}"
    );
    assert_eq!(
        report["runningJobs"].as_array().map(Vec::len),
        Some(0),
        "{report}"
    );

    let _ = lsp.request("shutdown", serde_json::json!({}));
    lsp.notify("exit", serde_json::json!({}));
}

/// catalog.json (dbt docs generate) backs source-column completion for
/// tables the YAML leaves undeclared — adapter-agnostic, always on,
/// landing with the next context refresh (03 §10).
#[test]
fn catalog_backs_source_columns() {
    let src = fixture_root().canonicalize().unwrap();
    let root = std::env::temp_dir().join(format!("cogitator-e2e-catalog-{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    copy_fixture(&src, &root);
    let _cleanup = RemoveOnDrop(root.clone());
    let target = root.join("target");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(
        target.join("catalog.json"),
        r#"{"nodes": {}, "sources": {"source.jaffle_shop.stripe.payments": {
            "metadata": {"type": "BASE TABLE", "schema": "main", "name": "payments"},
            "columns": {"amount": {"type": "INTEGER", "index": 2, "name": "amount"},
                        "payment_id": {"type": "BIGINT", "index": 1, "name": "payment_id"}},
            "stats": {}}}}"#,
    )
    .unwrap();

    let (mut lsp, _uri) = booted_in(&root, &[]);
    // a probe selecting from the undeclared-in-YAML source table
    let probe_uri = cogitator_lsp::fs_path_to_uri(&root.join("models/catalog_probe.sql"));
    lsp.notify(
        "textDocument/didOpen",
        serde_json::json!({"textDocument": {"uri": probe_uri, "languageId": "sql", "version": 1,
            "text": "select p. from {{ source('stripe', 'payments') }} p\n"}}),
    );
    // enrichment lands on a refresh; poll completion after the dot
    let mut labels: Vec<String> = Vec::new();
    for _ in 0..100 {
        let completion = lsp.request(
            "textDocument/completion",
            serde_json::json!({"textDocument": {"uri": probe_uri},
                               "position": {"line": 0, "character": 9}}),
        );
        labels = completion["result"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|i| i["label"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        if labels.iter().any(|l| l == "amount") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(labels.iter().any(|l| l == "amount"), "{labels:?}");
    assert!(labels.iter().any(|l| l == "payment_id"), "{labels:?}");

    // and hover on a catalog-backed column names the warehouse type
    lsp.notify(
        "textDocument/didChange",
        serde_json::json!({"textDocument": {"uri": probe_uri, "version": 2},
            "contentChanges": [{"text": "select p.amount from {{ source('stripe', 'payments') }} p\n"}]}),
    );
    let hover = lsp.request(
        "textDocument/hover",
        serde_json::json!({"textDocument": {"uri": probe_uri},
                           "position": {"line": 0, "character": 11}}),
    );
    assert!(
        hover["result"]["contents"]
            .as_str()
            .is_some_and(|c| c.contains("INTEGER (from catalog)")),
        "{hover}"
    );

    let _ = lsp.request("shutdown", serde_json::json!({}));
    lsp.notify("exit", serde_json::json!({}));
}
