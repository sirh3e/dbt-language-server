//! dbt.previewCte end to end: the "Preview rows" lenses, the query cut
//! at a CTE (rendered or compiled SQL), `dbt show --inline` through the
//! job registry, the table in the answer and as a markdown document.

mod harness;
use harness::jaffle;
use serde_json::json;

#[test]
fn every_cte_and_the_final_select_get_a_preview_lens() {
    let project = jaffle();
    let (mut lsp, uri) = project.booted();
    let reply = lsp.request(
        "textDocument/codeLens",
        json!({"textDocument": {"uri": uri}}),
    );
    let previews: Vec<serde_json::Value> = reply["result"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|l| l["command"]["command"] == "dbt.previewCte")
        .map(|l| l["command"]["arguments"][0]["cte"].clone())
        .collect();
    assert_eq!(
        previews.first(),
        Some(&json!("customers")),
        "first lens is the first CTE: {previews:?}"
    );
    assert!(previews.contains(&json!("payments")), "{previews:?}");
    assert_eq!(
        previews.last(),
        Some(&serde_json::Value::Null),
        "{previews:?}"
    );
    lsp.shutdown();
}

#[cfg(feature = "e2e")]
#[test]
fn a_cte_previews_through_dbt_show_inline() {
    let project = jaffle();
    let fake = harness::fake::install(harness::fake::FakeDbt::default());
    let envs = fake.envs();
    let mut env_refs: Vec<(&str, &str)> =
        envs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    env_refs.push(("FAKE_DBT_SHOW_ROWS", "9"));
    let (mut lsp, _) = project.booted_with(&env_refs, json!({}));
    // an unopened model whose Jinja the server renders on its own
    let uri = stg_orders(&project);

    let reply = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.previewCte", "arguments": [{"uri": uri, "cte": "source", "limit": 4}]}),
    );
    let result = &reply["result"];
    assert_eq!(result["model"], "stg_orders", "{reply}");
    assert_eq!(result["cte"], "source", "{reply}");
    // no compiled file in the fixture: the server rendered the Jinja
    assert_eq!(result["source"], "rendered", "{reply}");
    assert_eq!(result["columns"], json!(["n", "sql"]), "{reply}");
    assert_eq!(result["rows"].as_array().map(Vec::len), Some(4), "{reply}");
    assert_eq!(result["truncated"], true, "{reply}");
    let sql = result["sql"].as_str().unwrap();
    assert!(
        sql.ends_with("select * from source limit 5") && !sql.contains("renamed AS"),
        "{sql}"
    );
    assert_eq!(
        result["rows"][0][1],
        json!(sql),
        "the fake echoes the query it got"
    );
    assert!(
        result["table"].as_str().unwrap().contains("| n | sql |"),
        "{reply}"
    );

    let shown = lsp.wait_server_request("window/showDocument", |frame| {
        frame["params"]["uri"]
            .as_str()
            .is_some_and(|u| u.ends_with("stg_orders.source.md"))
    });
    assert!(shown["params"]["uri"].as_str().unwrap().contains("preview"));
    let table = std::fs::read_to_string(
        project
            .root
            .join("target/.cogitator/preview/stg_orders.source.md"),
    )
    .expect("preview table");
    assert!(table.contains("CTE `source`"), "{table}");

    let argv = fake.logged_args();
    assert!(
        argv.iter()
            .any(|line| line.starts_with("show --profiles-dir ")
                && line.contains("--inline WITH source AS")
                && line.contains("--limit 5 --output json")
                && !line.contains("--select")),
        "{argv:?}"
    );
    lsp.shutdown();
}

#[cfg(feature = "e2e")]
#[test]
fn a_current_compiled_file_is_preferred_and_the_cursor_picks_the_final_select() {
    let project = jaffle();
    let compiled = project
        .root
        .join("target/compiled/jaffle_shop/models/customers.sql");
    std::fs::create_dir_all(compiled.parent().unwrap()).unwrap();
    std::fs::write(
        &compiled,
        "with a as (select 1 as id), b as (select id from a) select * from b",
    )
    .unwrap();
    let fake = harness::fake::install(harness::fake::FakeDbt::default());
    let envs = fake.envs();
    let env_refs: Vec<(&str, &str)> = envs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let (mut lsp, uri) = project.booted_with(&env_refs, json!({}));

    // past every CTE of the buffer: the final select; no limit: preview.rows (20)
    let reply = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.previewCte",
               "arguments": [{"uri": uri, "position": {"line": 1000, "character": 0}}]}),
    );
    let result = &reply["result"];
    assert_eq!(result["source"], "compiled", "{reply}");
    assert_eq!(result["cte"], serde_json::Value::Null, "{reply}");
    assert_eq!(result["limit"], 20, "{reply}");
    assert_eq!(result["truncated"], false, "{reply}");
    assert!(
        result["sql"]
            .as_str()
            .is_some_and(|s| s.starts_with("select * from (WITH a AS") && s.ends_with("limit 21")),
        "{reply}"
    );

    let missing = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.previewCte", "arguments": [{"uri": uri, "cte": "nope"}]}),
    );
    let message = missing["error"]["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("no CTE 'nope'") && message.contains("a, b"),
        "{missing}"
    );
    lsp.shutdown();
}

#[cfg(feature = "e2e")]
#[test]
fn a_running_preview_cancels_by_token() {
    let project = jaffle();
    let fake = harness::fake::install(harness::fake::FakeDbt::sleeping(30_000));
    let envs = fake.envs();
    let env_refs: Vec<(&str, &str)> = envs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let (mut lsp, _) =
        project.booted_with(&env_refs, json!({"window": {"workDoneProgress": true}}));
    let uri = stg_orders(&project);

    let pending = lsp.request_nowait(
        "workspace/executeCommand",
        json!({"command": "dbt.previewCte", "arguments": [{"uri": uri, "cte": "renamed"}]}),
    );
    let begin = lsp.wait_notification("$/progress", |frame| {
        frame["params"]["value"]["title"]
            .as_str()
            .is_some_and(|t| t.starts_with("dbt show: stg_orders.renamed"))
    });
    let title = begin["params"]["value"]["title"].as_str().unwrap();
    let token: u64 = title
        .rsplit("[job ")
        .next()
        .and_then(|t| t.trim_end_matches(']').parse().ok())
        .expect("token in the title");
    let cancelled = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.cancel", "arguments": [{"token": token}]}),
    );
    assert_eq!(cancelled["result"]["token"], token, "{cancelled}");
    let answer = lsp.response_for(pending);
    assert_eq!(answer["error"]["code"], -32800, "{answer}");
    lsp.shutdown();
}

#[cfg(feature = "e2e")]
fn stg_orders(project: &harness::TestProject) -> String {
    cogitator_lsp::fs_path_to_uri(&project.root.join("models/staging/stg_orders.sql"))
}
