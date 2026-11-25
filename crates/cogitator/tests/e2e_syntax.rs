//! End-to-end suite (specs/rust-port/05 §7) — Jinja and SQL syntax
//! errors as you type: a didChange that breaks the buffer publishes the
//! diagnostic, the one that fixes it clears it, the pull path agrees,
//! and cogitator.yml `lint: { syntax: false }` turns the check off.

mod harness;
use harness::{jaffle, write_config};
use serde_json::json;

const GOOD: &str = "select customer_id from {{ ref('stg_customers') }}\n";
const BROKEN_JINJA: &str = "select customer_id from {{ ref('stg_customers') }}\n{% if x %}\n";
const BROKEN_SQL: &str = "select customer_id,, first_name from {{ ref('stg_customers') }}\n";

fn change(lsp: &mut harness::Lsp, uri: &str, version: i64, text: &str) {
    lsp.notify(
        "textDocument/didChange",
        json!({"textDocument": {"uri": uri, "version": version},
            "contentChanges": [{"text": text}]}),
    );
}

fn codes_of(diagnostics: &serde_json::Value) -> Vec<String> {
    diagnostics
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|d| d["code"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// The next publish for `uri` whose codes satisfy `pred`.
fn published(
    lsp: &mut harness::Lsp,
    uri: &str,
    pred: impl Fn(&[String]) -> bool,
) -> serde_json::Value {
    lsp.wait_notification("textDocument/publishDiagnostics", |frame| {
        frame["params"]["uri"] == uri && pred(&codes_of(&frame["params"]["diagnostics"]))
    })
}

fn pulled(lsp: &mut harness::Lsp, uri: &str) -> Vec<String> {
    let response = lsp.request(
        "textDocument/diagnostic",
        json!({"textDocument": {"uri": uri}}),
    );
    codes_of(&response["result"]["items"])
}

#[test]
fn syntax_errors_appear_and_clear_as_you_type() {
    let project = jaffle();
    let path = project.root.join("models/syntax_probe.sql");
    std::fs::write(&path, GOOD).unwrap();
    let (mut lsp, _) = project.booted();
    let uri = cogitator_lsp::fs_path_to_uri(&path);
    lsp.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": uri, "languageId": "sql", "version": 1, "text": GOOD}}),
    );
    published(&mut lsp, &uri, |codes| {
        !codes.iter().any(|c| c.ends_with("-syntax"))
    });

    change(&mut lsp, &uri, 2, BROKEN_JINJA);
    let frame = published(&mut lsp, &uri, |codes| {
        codes.iter().any(|c| c == "jinja-syntax")
    });
    let diagnostic = frame["params"]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == "jinja-syntax")
        .unwrap()
        .clone();
    assert_eq!(diagnostic["range"]["start"]["line"], 1, "{diagnostic}");
    assert_eq!(diagnostic["source"], "cogitator");
    assert_eq!(diagnostic["severity"], 1);
    let message = diagnostic["message"].as_str().unwrap();
    assert!(
        message.contains("Reason:") && message.contains("Solution:"),
        "{message}"
    );
    assert!(pulled(&mut lsp, &uri).iter().any(|c| c == "jinja-syntax"));

    change(&mut lsp, &uri, 3, BROKEN_SQL);
    published(&mut lsp, &uri, |codes| {
        codes.iter().any(|c| c == "sql-syntax")
    });
    assert!(pulled(&mut lsp, &uri).iter().any(|c| c == "sql-syntax"));

    change(&mut lsp, &uri, 4, GOOD);
    published(&mut lsp, &uri, |codes| {
        !codes.iter().any(|c| c.ends_with("-syntax"))
    });
    assert!(!pulled(&mut lsp, &uri)
        .iter()
        .any(|c| c.ends_with("-syntax")));

    // lint: turns the check off
    change(&mut lsp, &uri, 5, BROKEN_SQL);
    published(&mut lsp, &uri, |codes| {
        codes.iter().any(|c| c == "sql-syntax")
    });
    write_config(&project, &mut lsp, "lint:\n  syntax: false\n");
    lsp.wait_until("the syntax check switches off", |lsp| {
        (!pulled(lsp, &uri).iter().any(|c| c.ends_with("-syntax"))).then_some(())
    });
    lsp.shutdown();
}

/// An incremental model and a `{% for %}` loop: checked as dbt renders
/// them, on pull (which builds the renderer) and then on every push.
#[test]
fn incremental_models_and_loops_are_checked_as_dbt_renders_them() {
    const INCREMENTAL_BROKEN: &str = "{{ config(materialized='incremental') }}\n\
        select customer_id from {{ ref('stg_customers') }}\n\
        {% if is_incremental() %}\n\
        where customer_id > (select max(customer_id) from {{ this }}) and\n\
        {% endif %}\n";
    const INCREMENTAL_GOOD: &str = "{{ config(materialized='incremental') }}\n\
        select customer_id from {{ ref('stg_customers') }}\n\
        {% if is_incremental() %}\n\
        where customer_id > (select max(customer_id) from {{ this }})\n\
        {% endif %}\n";
    const LOOP_BROKEN: &str = "select\n\
        {% for c in ['customer_id', 'first_name'] %}max({{ c }}) as {{ c }} {% endfor %}\n\
        from {{ ref('stg_customers') }}\n";
    let project = jaffle();
    let path = project.root.join("models/incremental_probe.sql");
    std::fs::write(&path, INCREMENTAL_GOOD).unwrap();
    let (mut lsp, _) = project.booted();
    let uri = cogitator_lsp::fs_path_to_uri(&path);
    lsp.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": uri, "languageId": "sql", "version": 1, "text": INCREMENTAL_GOOD}}),
    );
    assert!(!pulled(&mut lsp, &uri)
        .iter()
        .any(|c| c.ends_with("-syntax")));

    change(&mut lsp, &uri, 2, INCREMENTAL_BROKEN);
    let response = lsp.request(
        "textDocument/diagnostic",
        json!({"textDocument": {"uri": uri}}),
    );
    let items = response["result"]["items"].as_array().unwrap().clone();
    let found = items
        .iter()
        .find(|d| d["code"] == "sql-syntax")
        .unwrap_or_else(|| panic!("{items:?}"));
    assert_eq!(found["range"]["start"]["line"], 3, "{found}");
    assert!(
        found["message"]
            .as_str()
            .unwrap()
            .contains("incremental run"),
        "{found}"
    );

    // the pull built the renderer: a push now checks loops as you type
    change(&mut lsp, &uri, 3, LOOP_BROKEN);
    published(&mut lsp, &uri, |codes| {
        codes.iter().any(|c| c == "sql-syntax")
    });
    change(&mut lsp, &uri, 4, INCREMENTAL_GOOD);
    published(&mut lsp, &uri, |codes| {
        !codes.iter().any(|c| c.ends_with("-syntax"))
    });
    lsp.shutdown();
}
