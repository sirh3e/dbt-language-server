//! End-to-end suite (specs/rust-port/05 §7) — this file's theme only;
//! the deterministic harness lives in tests/harness/.

mod harness;
use harness::jaffle;

/// The concurrency proof: a delayed dbt.modelInfo must NOT block the
/// hover sent right after it. The assertion is pure ORDER (the hover
/// reply arrives while the delayed one is still pending) — never a
/// wall-clock measurement (05 §7.1).
#[test]
fn slow_request_does_not_block_fast_ones() {
    let project = jaffle();
    let (mut lsp, uri) = project.booted_with(
        &[("COGITATOR_E2E_DELAY", "dbt.modelInfo=1500")],
        serde_json::json!({}),
    );
    let slow = lsp.request_nowait(
        "workspace/executeCommand",
        serde_json::json!({"command": "dbt.modelInfo",
                           "arguments": [{"model": "customers", "uri": ""}]}),
    );
    let fast = lsp.request_nowait(
        "textDocument/hover",
        serde_json::json!({"textDocument": {"uri": uri},
                           "position": {"line": 0, "character": 0}}),
    );
    // the hover must answer FIRST even though it was sent second
    let first = lsp.response_for(fast);
    assert!(first.get("result").is_some(), "{first}");
    let slow_reply = lsp.response_for(slow);
    assert_eq!(slow_reply["result"]["model"], "customers", "{slow_reply}");
    lsp.shutdown();
}

/// Real cancellation: the delayed request answers RequestCancelled
/// (-32800) and the server keeps serving.
#[test]
fn cancel_request_aborts_in_flight_work() {
    let project = jaffle();
    let (mut lsp, uri) = project.booted_with(
        &[("COGITATOR_E2E_DELAY", "dbt.modelInfo=1500")],
        serde_json::json!({}),
    );
    let slow = lsp.request_nowait(
        "workspace/executeCommand",
        serde_json::json!({"command": "dbt.modelInfo",
                           "arguments": [{"model": "customers", "uri": ""}]}),
    );
    lsp.notify("$/cancelRequest", serde_json::json!({"id": slow}));
    let cancelled = lsp.response_for(slow);
    assert_eq!(cancelled["error"]["code"], -32800, "{cancelled}");
    // and the server still answers afterwards
    let hover = lsp.request(
        "textDocument/hover",
        serde_json::json!({"textDocument": {"uri": uri},
                           "position": {"line": 0, "character": 0}}),
    );
    assert!(hover.get("result").is_some(), "{hover}");
    lsp.shutdown();
}

/// Silent drops are gone: a command with a non-object argument answers
/// InvalidParams (this request used to hang the client forever).
#[test]
fn malformed_command_argument_answers_invalid_params() {
    let project = jaffle();
    let (mut lsp, _uri) = project.booted();
    let bad = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({"command": "dbt.modelInfo", "arguments": [42]}),
    );
    assert_eq!(bad["error"]["code"], -32602, "{bad}");
    assert!(
        bad["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("argument OBJECT")),
        "{bad}"
    );
    lsp.shutdown();
}

/// The job registry (05 §9): dbt.runSelection answers a token for a
/// long-running fake dbt on PATH (cross-platform now), dbt.cancel
/// kills it, bad selectors and unknown tokens answer InvalidParams.
#[cfg(feature = "e2e")]
#[test]
fn dbt_jobs_start_and_cancel_by_token() {
    let project = jaffle();
    let fake = harness::fake::install(harness::fake::FakeDbt::sleeping(120_000));
    let envs = fake.envs();
    let env_refs: Vec<(&str, &str)> = envs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let (mut lsp, _uri) = project.booted_with(&env_refs, serde_json::json!({}));

    let started = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({"command": "dbt.runSelection",
                           "arguments": [{"selector": "tag:nightly", "kind": "run"}]}),
    );
    let token = started["result"]["token"].as_u64().expect("token");
    assert!(token >= 1, "{started}");

    let cancelled = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({"command": "dbt.cancel", "arguments": [{"token": token}]}),
    );
    assert_eq!(cancelled["result"]["token"], token, "{cancelled}");
    assert!(
        cancelled["result"]["label"]
            .as_str()
            .is_some_and(|l| l.contains("tag:nightly")),
        "{cancelled}"
    );

    // cancelling again: the token is gone
    let missing = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({"command": "dbt.cancel", "arguments": [{"token": token}]}),
    );
    assert_eq!(missing["error"]["code"], -32602, "{missing}");

    // argument-shaped selectors never reach argv
    let bad = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({"command": "dbt.runSelection",
                           "arguments": [{"selector": "--vars oops"}]}),
    );
    assert_eq!(bad["error"]["code"], -32602, "{bad}");
    assert!(
        bad["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("flag")),
        "{bad}"
    );

    lsp.shutdown();
}

/// dbt.preview: `dbt show --select <model> --limit N` through the job
/// registry; the fake dbt's stdout lands in target/.cogitator/preview/
/// (written atomically) and the exact argv is provable from its log.
#[cfg(feature = "e2e")]
#[test]
fn dbt_preview_captures_the_show_table() {
    let project = jaffle();
    let fake = harness::fake::install(harness::fake::FakeDbt::printing("| id | name |\n"));
    let envs = fake.envs();
    let env_refs: Vec<(&str, &str)> = envs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let (mut lsp, uri) = project.booted_with(&env_refs, serde_json::json!({}));

    let started = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({"command": "dbt.preview", "arguments": [{"uri": uri, "limit": 7}]}),
    );
    assert!(started["result"]["token"].as_u64().is_some(), "{started}");

    // completion is OBSERVABLE: the capture lands via showDocument
    let shown = lsp.wait_server_request("window/showDocument", |frame| {
        frame["params"]["uri"]
            .as_str()
            .is_some_and(|u| u.contains("preview") && u.contains("customers"))
    });
    let preview_uri = shown["params"]["uri"].as_str().unwrap();
    assert!(
        preview_uri.ends_with("customers.txt"),
        "preview capture path: {preview_uri}"
    );
    let preview = project.root.join("target/.cogitator/preview/customers.txt");
    let table = std::fs::read_to_string(&preview).expect("preview file");
    assert!(table.contains("| id | name |"), "{table}");

    let argv = fake.logged_args();
    assert!(
        argv.iter()
            .any(|line| line.starts_with("show --select customers") && line.contains("--limit 7")),
        "{argv:?}"
    );
    lsp.shutdown();
}
