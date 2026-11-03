//! End-to-end suite (specs/rust-port/05 §7) — this file's theme only;
//! the framed-LSP harness lives in tests/harness/.

mod harness;
#[allow(unused_imports)]
use harness::*;

/// The concurrency proof: a delayed dbt.modelInfo must NOT block the
/// hover sent right after it — three orders of magnitude of margin
/// (1500 ms floor vs a sub-ms handler), no polling.
#[test]
fn slow_request_does_not_block_fast_ones() {
    let (mut lsp, uri) = booted(&[("COGITATOR_E2E_DELAY", "dbt.modelInfo=1500")]);
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
    let _ = lsp.request("shutdown", serde_json::json!({}));
    lsp.notify("exit", serde_json::json!({}));
}

/// Real cancellation: the delayed request answers RequestCancelled
/// (-32800) and the server keeps serving.
#[test]
fn cancel_request_aborts_in_flight_work() {
    let (mut lsp, uri) = booted(&[("COGITATOR_E2E_DELAY", "dbt.modelInfo=1500")]);
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
    let _ = lsp.request("shutdown", serde_json::json!({}));
    lsp.notify("exit", serde_json::json!({}));
}

/// Silent drops are gone: a command with a non-object argument answers
/// InvalidParams (this request used to hang the client forever).
#[test]
fn malformed_command_argument_answers_invalid_params() {
    let (mut lsp, _uri) = booted(&[]);
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
    let _ = lsp.request("shutdown", serde_json::json!({}));
    lsp.notify("exit", serde_json::json!({}));
}

/// The job registry (05 §9): dbt.runSelection answers a token for a
/// fake long-running dbt on PATH, dbt.cancel kills it, bad selectors
/// and unknown tokens answer InvalidParams Guidance.
#[cfg(unix)]
#[test]
fn dbt_jobs_start_and_cancel_by_token() {
    use std::os::unix::fs::PermissionsExt;
    let bin_dir =
        std::env::temp_dir().join(format!("cogitator-e2e-fakedbt-{}", std::process::id()));
    std::fs::create_dir_all(&bin_dir).unwrap();
    let fake = bin_dir.join("dbt");
    std::fs::write(&fake, "#!/bin/sh\nsleep 30\n").unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let _cleanup = RemoveOnDrop(bin_dir.clone());
    let path_env = format!(
        "{}:{}",
        bin_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );

    let (mut lsp, _uri) = booted(&[("PATH", &path_env)]);

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

    let _ = lsp.request("shutdown", serde_json::json!({}));
    lsp.notify("exit", serde_json::json!({}));
}
