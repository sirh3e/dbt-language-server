//! Self-tests of the deterministic harness (05 §7.1): the boot barrier,
//! the notification waits, auto-replied server requests and the fake
//! dbt — proven here once so the themed suites can lean on them.

mod harness;
use harness::jaffle;

#[test]
fn boot_barrier_means_the_project_is_ready() {
    let project = jaffle();
    let (mut lsp, uri) = project.booted(); // panics if refresh 1 never lands

    // no proxy polling: after the barrier, hover over the ref answers
    let hover = lsp.request(
        "textDocument/hover",
        serde_json::json!({"textDocument": {"uri": uri},
                           "position": {"line": 2, "character": 24}}),
    );
    assert!(
        hover["result"]["contents"].as_str().is_some(),
        "hover answers right after the barrier: {hover}"
    );

    // the "context refreshed" overview log reached the editor stream,
    // whether it arrived before or after the frames consumed so far
    let log = lsp.wait_notification("window/logMessage", |frame| {
        frame["params"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("context refreshed"))
    });
    assert_eq!(log["params"]["type"], 3, "{log}");

    let status = lsp.shutdown();
    assert!(status.success(), "clean exit after shutdown: {status}");
}

#[cfg(feature = "e2e")]
#[test]
fn fake_dbt_runs_jobs_cross_platform() {
    let project = jaffle();
    let fake = harness::fake::install(harness::fake::FakeDbt::printing("done\n"));
    let envs = fake.envs();
    let env_refs: Vec<(&str, &str)> = envs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let (mut lsp, _uri) = project.booted_with(&env_refs, serde_json::json!({}));

    let started = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({"command": "dbt.runSelection",
                           "arguments": [{"selector": "tag:nightly", "kind": "run"}]}),
    );
    assert!(started["result"]["token"].as_u64().is_some(), "{started}");

    // progress is off (capabilities {}), so completion arrives as a
    // showMessage INFO — a real wait, not a stash scan
    let done = lsp.wait_notification("window/showMessage", |frame| {
        frame["params"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("dbt run succeeded"))
    });
    assert_eq!(done["params"]["type"], 3, "{done}");

    let args = fake.logged_args();
    assert!(
        args.iter().any(|line| line.contains("tag:nightly")),
        "the fake dbt saw the selection: {args:?}"
    );
    lsp.shutdown();
}
