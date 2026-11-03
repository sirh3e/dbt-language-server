//! End-to-end suite (specs/rust-port/05 §7) — this file's theme only;
//! the framed-LSP harness lives in tests/harness/.

mod harness;
#[allow(unused_imports)]
use harness::*;

/// live (the compiledSql wire cap), a parse failure keeps the old values
/// and surfaces as Guidance, and deleting the file reverts to launch
/// values — all driven through dbt.reloadConfig, never the 2 s poll.
#[test]
fn config_file_hot_reload() {
    let src = fixture_root().canonicalize().unwrap();
    let root = std::env::temp_dir().join(format!("cogitator-e2e-config-{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    copy_fixture(&src, &root);
    let _cleanup = RemoveOnDrop(root.clone());
    let config = root.join("cogitator.yml");
    std::fs::write(&config, "profiles:\n  target: dev\n").unwrap();

    let (mut lsp, _uri) = booted_in(&root, &[]);

    // 1. the boot pass loaded the file and applied profiles.target
    let status = status_until(&mut lsp, |s| {
        s["config"]["loaded"] == true && s["profiles"]["activeTarget"] == "dev"
    });
    assert!(
        status["config"]["path"]
            .as_str()
            .is_some_and(|p| p.ends_with("cogitator.yml")),
        "{status}"
    );
    assert!(status["config"]["error"].is_null(), "{status}");

    // 2. hot re-tune: a 0 MiB wire cap flips compiledSql to uri-only
    let compiled_dir = root.join("target/compiled/jaffle_shop/models");
    std::fs::create_dir_all(&compiled_dir).unwrap();
    std::fs::write(compiled_dir.join("customers.sql"), "select 1 as id\n").unwrap();
    std::fs::write(
        &config,
        "profiles:\n  target: dev\nlimits:\n  compiled_sql_mib: 0\n",
    )
    .unwrap();
    reload_config(&mut lsp);
    let mut capped = serde_json::Value::Null;
    for _ in 0..100 {
        let reply = lsp.request(
            "workspace/executeCommand",
            serde_json::json!({"command": "dbt.compiledSql",
                               "arguments": [{"model": "customers"}]}),
        );
        if reply["result"]["content"].is_null() && reply["result"]["uri"].is_string() {
            capped = reply;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(
        capped["result"]["uri"]
            .as_str()
            .is_some_and(|u| u.ends_with("customers.sql")),
        "cap never applied: {capped}"
    );
    // the cap explanation reached the client as a showMessage
    assert!(
        lsp.stash.iter().any(|m| {
            m["method"] == "window/showMessage"
                && m["params"]["message"]
                    .as_str()
                    .is_some_and(|t| t.contains("wire cap"))
        }),
        "{:?}",
        lsp.stash
    );

    // 3. a file that stops parsing: error surfaces, old values stay
    std::fs::write(&config, "profiles:\n  dir: [unclosed\n").unwrap();
    reload_config(&mut lsp);
    let broken = status_until(&mut lsp, |s| s["config"]["error"].is_string());
    assert_eq!(broken["profiles"]["activeTarget"], "dev", "{broken}");

    // 4. deleting the file reverts its keys to the launch values
    std::fs::remove_file(&config).unwrap();
    reload_config(&mut lsp);
    let reverted = status_until(&mut lsp, |s| s["config"]["loaded"] == false);
    assert!(reverted["config"]["path"].is_null(), "{reverted}");
    assert_eq!(reverted["profiles"]["activeTarget"], "", "{reverted}");

    let _ = lsp.request("shutdown", serde_json::json!({}));
    lsp.notify("exit", serde_json::json!({}));
}

/// cogitator.yml intelligence (03 §10): an open config file gets its
/// OWN diagnostics pass — unknown-key warnings from the shared schema,
/// never the dbt-properties pass that used to flag every root key.
#[test]
fn cogitator_yml_gets_config_intelligence() {
    let src = fixture_root().canonicalize().unwrap();
    let root = std::env::temp_dir().join(format!("cogitator-e2e-yamlcfg-{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    copy_fixture(&src, &root);
    let _cleanup = RemoveOnDrop(root.clone());
    let config = root.join("cogitator.yml");
    let text = "lint:\n  select_star: true\n  bogus: 1\n";
    std::fs::write(&config, text).unwrap();

    let (mut lsp, _uri) = booted_in(&root, &[]);
    let config_uri = cogitator_lsp::fs_path_to_uri(&config);
    lsp.notify(
        "textDocument/didOpen",
        serde_json::json!({"textDocument": {"uri": config_uri, "languageId": "yaml",
            "version": 1, "text": text}}),
    );
    let pulled = lsp.request(
        "textDocument/diagnostic",
        serde_json::json!({"textDocument": {"uri": config_uri}}),
    );
    let items = pulled["result"]["items"].as_array().expect("items");
    assert_eq!(items.len(), 1, "exactly the unknown-key warning: {pulled}");
    assert!(
        items[0]["message"]
            .as_str()
            .is_some_and(|m| m.contains("'bogus'") && m.contains("under lint:")),
        "{items:?}"
    );
    // completion inside lint: offers the checks, documented
    let completion = lsp.request(
        "textDocument/completion",
        serde_json::json!({"textDocument": {"uri": config_uri},
                           "position": {"line": 1, "character": 2}}),
    );
    let labels: Vec<&str> = completion["result"]
        .as_array()
        .expect("items")
        .iter()
        .filter_map(|i| i["label"].as_str())
        .collect();
    assert!(labels.contains(&"dead_models"), "{labels:?}");
    // hover documents the key from the schema table
    let hover = lsp.request(
        "textDocument/hover",
        serde_json::json!({"textDocument": {"uri": config_uri},
                           "position": {"line": 1, "character": 4}}),
    );
    assert!(
        hover["result"]["contents"]
            .as_str()
            .is_some_and(|c| c.contains("SELECT *")),
        "{hover}"
    );

    let _ = lsp.request("shutdown", serde_json::json!({}));
    lsp.notify("exit", serde_json::json!({}));
}

/// Opt-in lint, hot: no items while `lint:` is absent; writing
/// cogitator.yml with select_star: true and forcing a reload makes the
/// next pull carry the warning (with its column-list quickfix data).
#[test]
fn lint_toggles_hot_through_cogitator_yml() {
    let src = fixture_root().canonicalize().unwrap();
    let root = std::env::temp_dir().join(format!("cogitator-e2e-lint-{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    copy_fixture(&src, &root);
    let _cleanup = RemoveOnDrop(root.clone());

    let (mut lsp, _uri) = booted_in(&root, &[]);
    let probe_uri = cogitator_lsp::fs_path_to_uri(&root.join("models/mart_probe.sql"));
    lsp.notify(
        "textDocument/didOpen",
        serde_json::json!({"textDocument": {"uri": probe_uri, "languageId": "sql", "version": 1,
            "text": "select * from {{ ref('stg_customers') }}\n"}}),
    );
    let quiet = lsp.request(
        "textDocument/diagnostic",
        serde_json::json!({"textDocument": {"uri": probe_uri}}),
    );
    assert!(
        quiet["result"]["items"]
            .as_array()
            .is_some_and(|i| i.iter().all(|d| d["code"] != "lint-select-star")),
        "{quiet}"
    );

    std::fs::write(root.join("cogitator.yml"), "lint:\n  select_star: true\n").unwrap();
    reload_config(&mut lsp);
    let mut flagged = serde_json::Value::Null;
    for _ in 0..100 {
        let pulled = lsp.request(
            "textDocument/diagnostic",
            serde_json::json!({"textDocument": {"uri": probe_uri}}),
        );
        if let Some(item) = pulled["result"]["items"]
            .as_array()
            .and_then(|i| i.iter().find(|d| d["code"] == "lint-select-star"))
        {
            flagged = item.clone();
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(
        flagged["message"]
            .as_str()
            .is_some_and(|m| m.contains("SELECT *")),
        "lint never switched on: {flagged}"
    );
    assert!(
        flagged["data"]["suggestions"][0]
            .as_str()
            .is_some_and(|c| c.contains("customer_id")),
        "{flagged}"
    );

    let _ = lsp.request("shutdown", serde_json::json!({}));
    lsp.notify("exit", serde_json::json!({}));
}

/// Persisted scan cache (03 §10): the first session writes
/// target/.cogitator-cache; a fresh process seeds from it and its
/// first refresh serves stamp hits instead of re-reading the project.
#[test]
fn scan_cache_persists_across_sessions() {
    let src = fixture_root().canonicalize().unwrap();
    let root = std::env::temp_dir().join(format!("cogitator-e2e-cache-{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    copy_fixture(&src, &root);
    let _cleanup = RemoveOnDrop(root.clone());

    {
        let (mut lsp, _uri) = booted_in(&root, &[]);
        let _ = lsp.request("shutdown", serde_json::json!({}));
        lsp.notify("exit", serde_json::json!({}));
        let status = lsp.child.wait().unwrap();
        assert_eq!(status.code(), Some(0));
    }
    let cache = root.join("target/.cogitator-cache");
    assert!(cache.is_file(), "first session persisted the scan cache");
    let text = std::fs::read_to_string(&cache).unwrap();
    assert!(
        text.starts_with("{\"format\":1,"),
        "versioned header: {}",
        &text[..60.min(text.len())]
    );

    // second session: the first refresh hits instead of re-reading
    let (mut lsp, _uri) = booted_in(&root, &[]);
    let status = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({"command": "dbt.status", "arguments": [{}]}),
    );
    let hits = status["result"]["refresh"]["cacheHits"]
        .as_u64()
        .unwrap_or(0);
    assert!(hits > 0, "warm start served no scan-cache hits: {status}");

    let _ = lsp.request("shutdown", serde_json::json!({}));
    lsp.notify("exit", serde_json::json!({}));
}
