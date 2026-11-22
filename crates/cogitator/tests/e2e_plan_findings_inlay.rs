//! Plan findings as inlay hints (`perf.inline_findings`), end to end on
//! per-test copies of the jaffle fixture: off by default, shown with the
//! same severity rules as the diagnostics, merged with a row-count hint.

mod harness;
use harness::{jaffle, Lsp};
use serde_json::{json, Value};

const CARTESIAN: &str = "with a as (select 1 as x union all select 2 as x),\
     b as (select 3 as y union all select 4 as y)\
     select a.x from a, b\n";

/// `spare` is read by nothing: an unused CTE whose rows the catalog proves.
const SPARE: &str = "with recent as (select order_id from {{ ref('stg_orders') }}),\n\
                     spare as (select order_id from recent)\n\
                     select order_id from recent\n";

const CATALOG: &str = r#"{"metadata": {"generated_at": "2026-11-20T10:00:00Z"},
    "nodes": {"model.jaffle_shop.stg_orders": {
        "metadata": {"type": "BASE TABLE", "schema": "main", "name": "stg_orders"},
        "columns": {"order_id": {"type": "INTEGER", "index": 1, "name": "order_id"}},
        "stats": {"row_count": {"id": "row_count", "value": 1200, "include": true}}}},
    "sources": {}}"#;

fn open(lsp: &mut Lsp, uri: &str, text: &str) {
    lsp.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": uri, "languageId": "sql", "version": 1, "text": text}}),
    );
}

fn hints(lsp: &mut Lsp, uri: &str) -> Vec<Value> {
    let reply = lsp.request(
        "textDocument/inlayHint",
        json!({"textDocument": {"uri": uri},
               "range": {"start": {"line": 0, "character": 0}, "end": {"line": 99, "character": 0}}}),
    );
    reply["result"].as_array().cloned().unwrap_or_default()
}

/// A label as one string, a parted label's values joined.
fn shown(hint: &Value) -> String {
    match &hint["label"] {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts.iter().filter_map(|p| p["value"].as_str()).collect(),
        _ => String::new(),
    }
}

fn booted(
    config: &str,
    file: &str,
    text: &str,
    catalog: bool,
) -> (harness::TestProject, Lsp, String) {
    let project = jaffle();
    if catalog {
        let target = project.root.join("target");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("catalog.json"), CATALOG).unwrap();
    }
    std::fs::write(project.root.join("cogitator.yml"), config).unwrap();
    let path = project.root.join("models").join(file);
    std::fs::write(&path, text).unwrap();
    let uri = cogitator_lsp::fs_path_to_uri(&path);
    let (mut lsp, _) = project.booted();
    lsp.wait_status(|s| s["config"]["loaded"] == true);
    open(&mut lsp, &uri, text);
    (project, lsp, uri)
}

#[test]
fn findings_show_inline_when_switched_on() {
    let (_project, mut lsp, uri) = booted(
        "perf:\n  profile: recommended\n  inline_findings: true\n",
        "perf_probe.sql",
        CARTESIAN,
        false,
    );
    let status = lsp.wait_status(|s| s["perf"]["inlineFindings"] == true);
    assert_eq!(status["perf"]["inlineFindings"], true);
    let found = lsp.wait_until("the cartesian hint", |lsp| {
        hints(lsp, &uri)
            .into_iter()
            .find(|h| shown(h).contains("join.cartesian"))
    });
    assert_eq!(shown(&found), "⚠ join.cartesian", "{found}");
    let part = &found["label"][0];
    assert_eq!(part["command"]["command"], "dbt.queryPlan", "{found}");
    assert_eq!(part["command"]["arguments"][0]["uri"], uri.as_str());
    let tooltip = part["tooltip"].as_str().expect("a tooltip");
    assert!(
        tooltip.contains("rule: join.cartesian (warning)"),
        "{tooltip}"
    );
    lsp.shutdown();
}

#[test]
fn findings_stay_out_by_default_and_when_their_rule_is_off() {
    for config in [
        "perf:\n  profile: recommended\n",
        "perf:\n  profile: recommended\n  inline_findings: true\nperf_rules:\n  join.cartesian: off\n",
    ] {
        let (_project, mut lsp, uri) = booted(config, "perf_probe.sql", CARTESIAN, false);
        lsp.wait_status(|s| s["perf"]["profile"] == "recommended");
        // the diagnostic path plans the document; the hint path then reads the cache
        let shown: Vec<String> = hints(&mut lsp, &uri).iter().map(shown).collect();
        assert!(shown.iter().all(|l| !l.contains("join.cartesian")), "{config}: {shown:?}");
        lsp.shutdown();
    }
}

#[test]
fn a_finding_on_a_cte_joins_its_row_count() {
    let (_project, mut lsp, uri) = booted(
        "perf:\n  profile: recommended\n  inline_findings: true\n",
        "spare_probe.sql",
        SPARE,
        true,
    );
    lsp.wait_status(|s| s["perf"]["inlineFindings"] == true);
    let labels = lsp.wait_until("the merged spare hint", |lsp| {
        let labels: Vec<String> = hints(lsp, &uri).iter().map(shown).collect();
        labels
            .iter()
            .any(|l| l.contains("model.unused_cte"))
            .then_some(labels)
    });
    assert!(
        labels.contains(&"1,200 rows · ⚠ model.unused_cte".to_string()),
        "{labels:?}"
    );
    lsp.shutdown();
}
