//! dbt.diffAt, dbt.costReport and dbt.profileColumn end to end, with the
//! cost figures in the last-run lens and the profile in the hover.

mod harness;
use harness::jaffle;
use serde_json::json;

fn exec(lsp: &mut harness::Lsp, command: &str, argument: serde_json::Value) -> serde_json::Value {
    lsp.request(
        "workspace/executeCommand",
        json!({"command": command, "arguments": [argument]}),
    )
}

const RUN: &str = r#"{"metadata": {"generated_at": "2026-11-20T08:00:00Z"}, "results": [
  {"unique_id": "model.jaffle_shop.customers", "status": "success", "execution_time": 12.4,
   "adapter_response": {"bytes_processed": 3435973837, "slot_ms": 99, "rows_affected": 100}},
  {"unique_id": "model.jaffle_shop.orders", "status": "success", "execution_time": 2.0},
  {"unique_id": "model.jaffle_shop.stg_orders", "status": "success", "execution_time": 0.5},
  {"unique_id": "model.jaffle_shop.stg_customers", "status": "success", "execution_time": 0.2},
  {"unique_id": "model.jaffle_shop.stg_payments", "status": "success", "execution_time": 0.3}
]}"#;

#[test]
fn the_cost_report_ranks_the_run_and_the_lens_names_its_bytes() {
    let project = jaffle();
    let target = project.root.join("target");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(target.join("run_results.json"), RUN).unwrap();
    std::fs::write(
        project.root.join("history.json"),
        r#"[{"model": "orders", "total_bytes_billed": 2048}]"#,
    )
    .unwrap();
    std::fs::write(
        project.root.join("cogitator.yml"),
        "cost:\n  query_history: history.json\n",
    )
    .unwrap();
    let (mut lsp, uri) = project.booted();
    lsp.wait_status(|s| s["config"]["loaded"] == true);

    let lenses = lsp.request(
        "textDocument/codeLens",
        json!({"textDocument": {"uri": uri}}),
    );
    let titles: Vec<&str> = lenses["result"]
        .as_array()
        .expect("lenses")
        .iter()
        .filter_map(|l| l["command"]["title"].as_str())
        .collect();
    assert!(
        titles.contains(&"last run: OK in 12.4 s · 3.2 GB scanned · slowest 20%"),
        "{titles:?}"
    );

    let report = exec(&mut lsp, "dbt.costReport", json!({}))["result"].clone();
    let models = report["models"].as_array().expect("models");
    assert_eq!(models[0]["model"], "customers", "{report}");
    assert_eq!(models[0]["bytesProcessed"], 3_435_973_837u64, "{report}");
    assert_eq!(
        models[1]["bytesBilled"], 2048,
        "history filled it: {report}"
    );
    assert_eq!(report["historyRows"], 1, "{report}");
    let path = report["criticalPath"]["models"].as_array().expect("path");
    assert_eq!(path.last().unwrap(), "customers", "{report}");
    assert!(
        report["text"].as_str().unwrap().contains("critical path"),
        "{report}"
    );
    lsp.shutdown();
}

#[test]
fn diff_at_the_last_run_and_without_git() {
    let project = jaffle();
    let target = project.root.join("target");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(
        target.join("manifest.json"),
        r#"{"nodes": {"model.jaffle_shop.stg_orders": {"resource_type": "model",
            "name": "stg_orders", "package_name": "jaffle_shop",
            "original_file_path": "models/staging/stg_orders.sql",
            "raw_code": "select 1 as order_id",
            "compiled_code": "select 1 as order_id"}}}"#,
    )
    .unwrap();
    let (mut lsp, _) = project.booted();
    let uri = cogitator_lsp::fs_path_to_uri(&project.root.join("models/staging/stg_orders.sql"));

    let reply = exec(
        &mut lsp,
        "dbt.diffAt",
        json!({"uri": uri, "rev": "last-run"}),
    );
    let result = &reply["result"];
    assert_eq!(result["what"], "sql", "{reply}");
    assert_eq!(result["identical"], false, "{reply}");
    assert!(
        result["diff"]
            .as_str()
            .unwrap()
            .contains("-select 1 as order_id"),
        "{reply}"
    );

    let reply = exec(
        &mut lsp,
        "dbt.diffAt",
        json!({"uri": uri, "rev": "last-run", "what": "lineage", "show": true}),
    );
    assert!(reply["result"]["columns"].is_object(), "{reply}");
    lsp.wait_server_request("window/showDocument", |frame| {
        frame["params"]["uri"]
            .as_str()
            .is_some_and(|u| u.ends_with("stg_orders@last-run.lineage.diff"))
    });

    // the fixture copy is no git repository (or git is absent): guidance
    let reply = exec(&mut lsp, "dbt.diffAt", json!({"uri": uri, "rev": "HEAD"}));
    let message = reply["error"]["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("git") || message.contains("revision"),
        "{reply}"
    );
    let reply = exec(
        &mut lsp,
        "dbt.diffAt",
        json!({"uri": uri, "what": "compild"}),
    );
    assert!(
        reply["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("compiled"),
        "{reply}"
    );
    lsp.shutdown();
}

#[test]
fn diff_at_a_git_revision() {
    let project = jaffle();
    let root = project.root.clone();
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@e",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(&root)
            .output()
    };
    if git(&["init", "-q"])
        .map(|o| !o.status.success())
        .unwrap_or(true)
    {
        eprintln!("git unavailable: skipped");
        return;
    }
    git(&["add", "-A"]).unwrap();
    assert!(git(&["commit", "-qm", "base"]).unwrap().status.success());
    let model = root.join("models/staging/stg_orders.sql");
    let mut text = std::fs::read_to_string(&model).unwrap();
    text.push_str("\n-- changed after the commit\n");
    std::fs::write(&model, &text).unwrap();

    let (mut lsp, _) = project.booted();
    let uri = cogitator_lsp::fs_path_to_uri(&model);
    let reply = exec(&mut lsp, "dbt.diffAt", json!({"uri": uri}));
    let result = &reply["result"];
    assert!(
        result["rev"].as_str().unwrap().starts_with("HEAD ("),
        "{reply}"
    );
    assert!(
        result["diff"]
            .as_str()
            .unwrap()
            .contains("+-- changed after the commit"),
        "{reply}"
    );
    let reply = exec(
        &mut lsp,
        "dbt.diffAt",
        json!({"uri": uri, "rev": "no-such-rev"}),
    );
    assert!(
        reply["error"]["message"]
            .as_str()
            .unwrap()
            .contains("not a revision"),
        "{reply}"
    );
    lsp.shutdown();
}

#[test]
fn profiling_is_off_by_default() {
    let project = jaffle();
    let (mut lsp, uri) = project.booted();
    let reply = exec(
        &mut lsp,
        "dbt.profileColumn",
        json!({"uri": uri, "position": {"line": 58, "character": 20}}),
    );
    assert!(
        reply["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("warehouse.profile"),
        "{reply}"
    );
    lsp.shutdown();
}

#[cfg(all(feature = "e2e", not(feature = "duckdb-parser")))]
#[test]
fn a_column_profiles_through_dbt_show_and_the_hover_shows_it() {
    let project = jaffle();
    std::fs::write(
        project.root.join("cogitator.yml"),
        "warehouse:\n  profile: true\n",
    )
    .unwrap();
    let show = r#"{"show": [
        {"kind": "stats", "value": "Aiden", "value2": "Zoe", "n": 100, "distinct_count": 79, "null_count": 5},
        {"kind": "top", "value": "Mary", "value2": null, "n": 4, "distinct_count": null, "null_count": null},
        {"kind": "top", "value": "John", "value2": null, "n": 2, "distinct_count": null, "null_count": null}
    ]}"#;
    let fake = harness::fake::install(harness::fake::FakeDbt::printing(show));
    let envs = fake.envs();
    let env_refs: Vec<(&str, &str)> = envs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let (mut lsp, uri) = project.booted_with(&env_refs, json!({}));
    lsp.wait_status(|s| s["config"]["loaded"] == true);
    // `first_name` in the final select (line 59)
    let position = json!({"line": 58, "character": 20});
    let reply = exec(
        &mut lsp,
        "dbt.profileColumn",
        json!({"uri": uri, "position": position}),
    );
    let profile = &reply["result"]["profile"];
    assert_eq!(profile["column"], "first_name", "{reply}");
    assert_eq!(profile["distinct"], 79, "{reply}");
    assert_eq!(profile["engine"], "dbt show", "{reply}");
    assert!(
        reply["result"]["hover"]
            .as_str()
            .unwrap()
            .contains("null 5.0%"),
        "{reply}"
    );
    let argv = fake.logged_args();
    assert!(
        argv.iter().any(
            |l| l.contains("--inline with sampled as") && l.contains("using sample 10000 rows")
        ),
        "{argv:?}"
    );
    let hovered = lsp.wait_until("the hover shows the profile", |lsp| {
        let hover = lsp.request(
            "textDocument/hover",
            json!({"textDocument": {"uri": uri}, "position": position}),
        );
        let text = hover["result"]["contents"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        text.contains("distinct 79").then_some(text)
    });
    assert!(hovered.contains("top values █▅"), "{hovered}");
    let actions = lsp.request(
        "textDocument/codeAction",
        json!({"textDocument": {"uri": uri}, "range": {"start": position, "end": position},
               "context": {"diagnostics": []}}),
    );
    assert!(
        actions["result"].as_array().is_some_and(|a| a
            .iter()
            .any(|x| x["title"] == "Profile column 'first_name'")),
        "{actions}"
    );
    lsp.shutdown();
}
