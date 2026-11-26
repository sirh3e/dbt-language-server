//! The correctness checks that report under the default profile, end to
//! end on a jaffle copy: `join.fanout` with its YAML quick fix, a
//! `type-*` diagnostic, and `inlay.types` once switched on.

mod harness;
use harness::{jaffle, write_config, Lsp};
use serde_json::{json, Value};

/// stg_payments.payment_method is documented but not unique.
const FANOUT: &str = "select o.order_id, p.amount\nfrom {{ ref('stg_orders') }} as o\njoin {{ ref('stg_payments') }} as p on o.status = p.payment_method\n";

const TYPED: &str = "select\n    cast(order_id as bigint) as order_id,\n    cast(order_date as date) as first_day,\n    'web' as channel\nfrom {{ ref('stg_orders') }}\nwhere cast(status as varchar) > cast(order_date as date)\n";

fn open(lsp: &mut Lsp, uri: &str, text: &str) {
    lsp.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": uri, "languageId": "sql", "version": 1, "text": text}}),
    );
}

fn pull(lsp: &mut Lsp, uri: &str) -> Vec<Value> {
    let reply = lsp.request(
        "textDocument/diagnostic",
        json!({"textDocument": {"uri": uri}}),
    );
    reply["result"]["items"].as_array().expect("items").clone()
}

#[test]
fn join_fanout_reports_by_default_and_offers_the_unique_test() {
    let project = jaffle();
    let path = project.root.join("models/fanout_probe.sql");
    std::fs::write(&path, FANOUT).unwrap();
    let uri = cogitator_lsp::fs_path_to_uri(&path);
    let (mut lsp, _) = project.booted();
    open(&mut lsp, &uri, FANOUT);
    let fanout = lsp.wait_until("join.fanout reports", |lsp| {
        pull(lsp, &uri)
            .into_iter()
            .find(|d| d["code"] == "join.fanout")
    });
    assert_eq!(fanout["severity"], 4, "a hint: {fanout}");
    assert!(
        fanout["message"]
            .as_str()
            .unwrap()
            .starts_with("this join may duplicate rows of stg_orders: payment_method is not known to be unique in stg_payments"),
        "{fanout}"
    );
    let reply = lsp.request(
        "textDocument/codeAction",
        json!({"textDocument": {"uri": uri}, "range": fanout["range"],
               "context": {"diagnostics": [fanout]}}),
    );
    let actions = reply["result"].as_array().expect("actions");
    let titles: Vec<&str> = actions.iter().filter_map(|a| a["title"].as_str()).collect();
    assert!(
        titles.contains(&"Add a unique test on stg_payments.payment_method"),
        "{titles:?}"
    );
    assert!(
        titles
            .iter()
            .any(|t| t.starts_with("Deduplicate stg_payments on payment_method")),
        "{titles:?}"
    );
    let yaml = actions
        .iter()
        .find(|a| a["title"] == "Add a unique test on stg_payments.payment_method")
        .unwrap();
    let edit = &yaml["edit"]["documentChanges"][0];
    assert!(
        edit["textDocument"]["uri"]
            .as_str()
            .unwrap()
            .ends_with("staging/schema.yml"),
        "{edit}"
    );
    assert!(
        edit["edits"][0]["newText"]
            .as_str()
            .unwrap()
            .contains("- unique"),
        "{edit}"
    );
    lsp.shutdown();
}

#[test]
fn a_type_check_reports_with_a_type_code_and_inlay_types_switch_on() {
    let project = jaffle();
    let path = project.root.join("models/typed_probe.sql");
    std::fs::write(&path, TYPED).unwrap();
    let uri = cogitator_lsp::fs_path_to_uri(&path);
    let (mut lsp, _) = project.booted();
    open(&mut lsp, &uri, TYPED);
    let typed = lsp.wait_until("the type check reports", |lsp| {
        pull(lsp, &uri)
            .into_iter()
            .find(|d| d["code"] == "type-text-temporal-compare")
    });
    assert_eq!(typed["severity"], 2, "both sides proven: {typed}");

    let hints = |lsp: &mut Lsp| -> Vec<String> {
        let reply = lsp.request(
            "textDocument/inlayHint",
            json!({"textDocument": {"uri": uri},
                   "range": {"start": {"line": 0, "character": 0}, "end": {"line": 99, "character": 0}}}),
        );
        reply["result"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|h| h["label"].as_str().map(str::to_string))
            .filter(|l| l.starts_with(": "))
            .collect()
    };
    assert!(hints(&mut lsp).is_empty(), "off by default");
    write_config(&project, &mut lsp, "inlay:\n  types: true\n");
    let shown = lsp.wait_until("type hints appear", |lsp| {
        let shown = hints(lsp);
        (!shown.is_empty()).then_some(shown)
    });
    assert!(shown.contains(&": bigint".to_string()), "{shown:?}");
    assert!(shown.contains(&": date".to_string()), "{shown:?}");
    lsp.shutdown();
}
