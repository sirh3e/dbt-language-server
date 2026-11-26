//! Inline column lineage hints (`inlay.lineage`), end to end: the first
//! request answers from the cache alone, the lineage is computed off the
//! dispatcher, and `workspace/inlayHint/refresh` brings the hints, each
//! upstream column a label part pointing at its definition.

mod harness;
use harness::{jaffle, write_config, Lsp};
use serde_json::{json, Value};

const PROBE: &str =
    "with p as (\n    select payment_id, amount from {{ ref('stg_payments') }}\n)\n\
                     select\n    payment_id,\n    amount * 100 as amount_cents\nfrom p\n";

fn open(lsp: &mut Lsp, uri: &str, text: &str) {
    lsp.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": uri, "languageId": "sql", "version": 1, "text": text}}),
    );
}

/// The parted hints' labels, with each hint's parts.
fn lineage_hints(lsp: &mut Lsp, uri: &str) -> Vec<(String, Value)> {
    let reply = lsp.request(
        "textDocument/inlayHint",
        json!({"textDocument": {"uri": uri},
               "range": {"start": {"line": 0, "character": 0}, "end": {"line": 99, "character": 0}}}),
    );
    reply["result"]
        .as_array()
        .map(|hints| {
            hints
                .iter()
                .filter_map(|h| {
                    let parts = h["label"].as_array()?;
                    let label: String = parts.iter().filter_map(|p| p["value"].as_str()).collect();
                    label.starts_with('←').then(|| (label, h.clone()))
                })
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn lineage_hints_arrive_after_a_refresh_and_point_upstream() {
    let project = jaffle();
    let path = project.root.join("models/lineage_probe.sql");
    std::fs::write(&path, PROBE).unwrap();
    let uri = cogitator_lsp::fs_path_to_uri(&path);
    let capabilities = json!({"workspace": {"inlayHint": {"refreshSupport": true}}});
    let (mut lsp, _) = project.booted_with(&[], capabilities);
    open(&mut lsp, &uri, PROBE);
    // the first request starts the computation; the refresh says it landed
    let _ = lineage_hints(&mut lsp, &uri);
    let refresh = lsp.wait_server_request("workspace/inlayHint/refresh", |_| true);
    assert!(
        refresh.get("params").is_none(),
        "no params member: {refresh}"
    );
    let hints = lineage_hints(&mut lsp, &uri);
    let labels: Vec<&str> = hints.iter().map(|(l, _)| l.as_str()).collect();
    assert_eq!(
        labels,
        vec![
            "← stg_payments.payment_id",
            "← stg_payments.amount · derived"
        ],
        "{hints:#?}"
    );
    let (_, derived) = &hints[1];
    assert_eq!(derived["position"], json!({"line": 5, "character": 32}));
    let part = &derived["label"][1];
    assert!(
        part["location"]["uri"]
            .as_str()
            .unwrap()
            .ends_with("stg_payments.sql"),
        "{part}"
    );
    lsp.shutdown();
}

#[test]
fn lineage_hints_switch_off_and_on_without_refresh_support() {
    let project = jaffle();
    let path = project.root.join("models/lineage_probe.sql");
    std::fs::write(&path, PROBE).unwrap();
    let uri = cogitator_lsp::fs_path_to_uri(&path);
    let (mut lsp, _) = project.booted();
    write_config(&project, &mut lsp, "inlay:\n  lineage: false\n");
    open(&mut lsp, &uri, PROBE);
    // the lineage cached another way: only the switch keeps the hints out
    let _ = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.columnLineage", "arguments": [{"uri": uri}]}),
    );
    assert!(lineage_hints(&mut lsp, &uri).is_empty());
    write_config(
        &project,
        &mut lsp,
        "inlay:\n  lineage: true\n  lineage_max_len: 20\n",
    );
    // no refresh support: a later request finds the cache warm
    let shown = lsp.wait_until("lineage hints once switched on", |lsp| {
        let hints = lineage_hints(lsp, &uri);
        (!hints.is_empty()).then_some(hints)
    });
    assert!(
        shown.iter().all(|(l, _)| l.chars().count() <= 20),
        "{shown:#?}"
    );
    assert!(shown.iter().any(|(l, _)| l.contains('…')), "{shown:#?}");
    lsp.shutdown();
}
