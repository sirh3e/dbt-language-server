//! End-to-end suite (specs/rust-port/05 §7) — dbt.select over the wire:
//! a selection's nodes and counts, completion's vocabulary, an inline
//! `--exclude`, and an error with its span.

mod harness;
use harness::jaffle;

#[test]
fn dbt_select_answers_a_selection_over_the_wire() {
    let project = jaffle();
    let (mut lsp, uri) = project.booted();
    let select = |lsp: &mut harness::Lsp, arg: serde_json::Value| {
        lsp.request(
            "workspace/executeCommand",
            serde_json::json!({"command": "dbt.select", "arguments": [arg]}),
        )["result"]
            .clone()
    };

    let answer = select(
        &mut lsp,
        serde_json::json!({"uri": uri, "select": "+customers", "vocabulary": true}),
    );
    let names: Vec<&str> = answer["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .filter_map(|n| n["name"].as_str())
        .collect();
    for want in ["customers", "stg_customers", "stg_orders", "raw_customers"] {
        assert!(names.contains(&want), "{want} in {names:?}");
    }
    let customers = answer["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["name"] == "customers")
        .unwrap();
    assert_eq!(
        customers["uniqueId"], "model.jaffle_shop.customers",
        "{customers}"
    );
    assert_eq!(customers["resourceType"], "model");
    assert_eq!(customers["graphId"], "model:customers");
    assert!(answer["counts"]["model"].as_u64().unwrap() >= 3, "{answer}");
    assert!(answer["error"].is_null(), "{answer}");
    let tags = answer["vocabulary"]["values"]["tag"]
        .as_array()
        .expect("tags");
    assert!(tags.iter().any(|t| t == "nightly"), "{tags:?}");

    let only_models = select(
        &mut lsp,
        serde_json::json!({"uri": uri, "select": "+customers --exclude resource_type:test resource_type:seed"}),
    );
    assert!(
        only_models["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|n| n["resourceType"] == "model"),
        "{only_models}"
    );
    assert!(
        only_models.get("vocabulary").is_none(),
        "asked for no vocabulary"
    );

    let wrong = select(
        &mut lsp,
        serde_json::json!({"uri": uri, "select": "tag:nightly bogus:x"}),
    );
    assert_eq!(wrong["error"]["start"], 12, "{wrong}");
    assert_eq!(wrong["error"]["end"], 18, "{wrong}");
    assert!(wrong["nodes"].as_array().unwrap().is_empty());
}

/// What dbt.select listed is what dbt runs: dbt.runSelection passes the
/// selection's exclusion on as dbt's own `--exclude`, and refuses one
/// that is argument-shaped as it refuses such a selector.
#[cfg(feature = "e2e")]
#[test]
fn a_selection_runs_with_its_exclusion() {
    let project = jaffle();
    let fake = harness::fake::install(harness::fake::FakeDbt::printing("ok\n"));
    let envs = fake.envs();
    let env_refs: Vec<(&str, &str)> = envs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let (mut lsp, uri) = project.booted_with(&env_refs, serde_json::json!({}));

    let started = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({"command": "dbt.runSelection", "arguments": [{
            "uri": uri, "selector": "+customers", "exclude": "resource_type:test", "kind": "build"}]}),
    );
    assert!(started["result"]["token"].as_u64().is_some(), "{started}");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let argv = loop {
        let argv = fake.logged_args();
        if !argv.is_empty() || std::time::Instant::now() > deadline {
            break argv;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    assert!(
        argv.iter()
            .any(|line| line.starts_with("build --select +customers")
                && line.contains("--exclude resource_type:test")),
        "{argv:?}"
    );

    let bad = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({"command": "dbt.runSelection", "arguments": [{
            "uri": uri, "selector": "tag:a", "exclude": "--vars oops"}]}),
    );
    assert_eq!(bad["error"]["code"], -32602, "{bad}");
    lsp.shutdown();
}
