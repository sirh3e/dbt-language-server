//! End-to-end suite (specs/rust-port/05 §7) — dispatched macros answer
//! textDocument/implementation from a call, a dispatch and a definition,
//! hover names the implementation the active adapter picks, and macro
//! calls are checked against the macro's parameters (on by default,
//! off through cogitator.yml `lint:`).

mod harness;
use harness::{jaffle, write_config};
use serde_json::json;

fn position_of(text: &str, needle: &str, offset: u32) -> serde_json::Value {
    let at = text.find(needle).expect("needle");
    let line = text[..at].matches('\n').count();
    let col = at - text[..at].rfind('\n').map_or(0, |i| i + 1);
    json!({"line": line, "character": col as u32 + offset})
}

const DISPATCHING: &str = concat!(
    "{% macro my_x(a) %}{{ return(adapter.dispatch('my_x', 'jaffle_shop')(a)) }}{% endmacro %}\n",
    "{% macro default__my_x(a) %}{{ a }}{% endmacro %}\n",
    "{% macro duckdb__my_x(a) %}{{ a }}{% endmacro %}\n",
);

const PROBE: &str = concat!(
    "select {{ my_x(1) }} as a,\n",
    "  {{ full_name('a', 'b', 'c') }} as b,\n",
    "  {{ full_name('a', lst_name='b') }} as c,\n",
    "  {{ adapter.dispatch('nowhere', 'jaffle_shop')() }} as d\n",
    "from {{ ref('customers') }}\n",
);

fn codes(lsp: &mut harness::Lsp, uri: &str) -> Vec<String> {
    let pulled = lsp.request(
        "textDocument/diagnostic",
        json!({"textDocument": {"uri": uri}}),
    );
    pulled["result"]["items"]
        .as_array()
        .expect("items")
        .iter()
        .filter_map(|d| d["code"].as_str().map(str::to_string))
        .collect()
}

#[test]
fn dispatched_macros_answer_implementation_and_hover() {
    let project = jaffle();
    std::fs::write(project.root.join("macros/my_x.sql"), DISPATCHING).unwrap();
    std::fs::write(project.root.join("models/macro_probe.sql"), PROBE).unwrap();
    let (mut lsp, _) = project.booted();

    let probe_uri = cogitator_lsp::fs_path_to_uri(&project.root.join("models/macro_probe.sql"));
    lsp.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": probe_uri, "languageId": "sql", "version": 1,
            "text": PROBE}}),
    );
    let macros_path = project.root.join("macros/my_x.sql");
    let macros_uri = cogitator_lsp::fs_path_to_uri(&macros_path);
    lsp.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": macros_uri, "languageId": "sql", "version": 1,
            "text": DISPATCHING}}),
    );

    // from a call, the dispatch and the definition: both implementations
    for (uri, text, needle, offset) in [
        (&probe_uri, PROBE, "my_x(1)", 1),
        (&macros_uri, DISPATCHING, "dispatch('my_x'", 2),
        (&macros_uri, DISPATCHING, "my_x(a) %}{{ return", 1),
    ] {
        let response = lsp.request(
            "textDocument/implementation",
            json!({"textDocument": {"uri": uri}, "position": position_of(text, needle, offset)}),
        );
        let locations = response["result"].as_array().expect("locations");
        assert_eq!(locations.len(), 2, "{needle}: {response}");
        assert!(
            locations
                .iter()
                .all(|l| l["uri"].as_str().is_some_and(|u| u.ends_with("my_x.sql"))),
            "{response}"
        );
    }

    let hover = lsp.request(
        "textDocument/hover",
        json!({"textDocument": {"uri": probe_uri}, "position": position_of(PROBE, "my_x(1)", 1)}),
    );
    let contents = hover["result"]["contents"].to_string();
    assert!(contents.contains("Dispatches to `jaffle_shop."), "{hover}");

    // the call checks are on without any configuration
    let found = codes(&mut lsp, &probe_uri);
    for code in [
        "macro-arg-count",
        "macro-unknown-kwarg",
        "dispatch-no-default",
    ] {
        assert!(found.iter().any(|c| c == code), "{code} missing: {found:?}");
    }

    // and lint: turns them off
    write_config(
        &project,
        &mut lsp,
        "lint:\n  macro_calls: false\n  dispatch: false\n",
    );
    lsp.wait_until("the macro checks switch off", |lsp| {
        let found = codes(lsp, &probe_uri);
        found
            .iter()
            .all(|c| !c.starts_with("macro-") && c != "dispatch-no-default")
            .then_some(())
    });
    lsp.shutdown();
}
