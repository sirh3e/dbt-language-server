//! Column completion over the wire: the relations in scope's columns,
//! qualified and bare, ranked above every global by `sortText`, with
//! `filterText` and `preselect`; and a model without documented columns
//! read from disk for its lineage.

mod harness;
use harness::jaffle;
use serde_json::{json, Value};

/// The items at (line, character) of `text`, sorted by `sortText`.
fn complete(lsp: &mut harness::client::Lsp, uri: &str, text: &str, character: u32) -> Vec<Value> {
    lsp.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": uri, "languageId": "sql", "version": 1, "text": text}}),
    );
    let answer = lsp.request(
        "textDocument/completion",
        json!({"textDocument": {"uri": uri}, "position": {"line": 0, "character": character}}),
    );
    let mut items = answer["result"]
        .as_array()
        .expect("completion items")
        .clone();
    items.sort_by(|a, b| {
        a["sortText"]
            .as_str()
            .unwrap_or_default()
            .cmp(b["sortText"].as_str().unwrap_or_default())
    });
    items
}

fn labels(items: &[Value], n: usize) -> Vec<&str> {
    items
        .iter()
        .take(n)
        .filter_map(|i| i["label"].as_str())
        .collect()
}

#[test]
fn columns_in_scope_rank_qualified_where_ambiguous() {
    let project = jaffle();
    std::fs::write(
        project.root.join("models/payment_totals.sql"),
        "select order_id, sum(amount) as total from {{ ref('stg_payments') }} group by 1\n",
    )
    .unwrap();
    let (mut lsp, _) = project.booted();
    let probe = cogitator_lsp::fs_path_to_uri(&project.root.join("models/probe.sql"));

    let text = "select  from {{ ref('orders') }} o join {{ ref('customers') }} c on o.customer_id = c.customer_id";
    let items = complete(&mut lsp, &probe, text, 7);
    assert_eq!(
        labels(&items, 3),
        ["c.customer_id", "o.customer_id", "amount"],
        "ambiguous names qualified first"
    );
    assert_eq!(items[0]["preselect"], true);
    assert_eq!(items[0]["filterText"], "customer_id");
    assert_eq!(items[0]["detail"], "customers");
    assert_eq!(items[0]["kind"], 5, "Field");
    let first_global = items
        .iter()
        .position(|i| i["kind"] != 5)
        .expect("globals follow");
    assert!(
        items[..first_global]
            .iter()
            .any(|i| i["label"] == "o.order_id"),
        "qualified columns before every global: {:?}",
        labels(&items, first_global + 1)
    );

    let undocumented = complete(
        &mut lsp,
        &cogitator_lsp::fs_path_to_uri(&project.root.join("models/probe2.sql")),
        "select t. from {{ ref('payment_totals') }} t",
        9,
    );
    assert_eq!(labels(&undocumented, 2), ["order_id", "total"]);
    lsp.shutdown();
}

/// The column name resolver over the wire: once the schema job has
/// published, a model that is `select *` of an undocumented `select *` of
/// a model offers the columns at the bottom of the chain, with their
/// types — what its own lineage (`*` left unexpanded) never could.
#[test]
fn columns_resolve_through_a_chain_of_select_stars() {
    let project = jaffle();
    for (name, sql) in [
        ("star_payments", "select * from {{ ref('stg_payments') }}\n"),
        ("star_star", "select * from {{ ref('star_payments') }}\n"),
    ] {
        std::fs::write(project.root.join(format!("models/{name}.sql")), sql).unwrap();
    }
    let (mut lsp, _) = project.booted();
    let probe = cogitator_lsp::fs_path_to_uri(&project.root.join("models/probe3.sql"));
    let text = "select t. from {{ ref('star_star') }} t";
    lsp.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": probe, "languageId": "sql", "version": 1, "text": text}}),
    );
    let columns = lsp.wait_until("the schema job resolves star_star's columns", |lsp| {
        let answer = lsp.request(
            "textDocument/completion",
            json!({"textDocument": {"uri": probe}, "position": {"line": 0, "character": 9}}),
        );
        let fields: Vec<Value> = answer["result"]
            .as_array()?
            .iter()
            .filter(|i| i["kind"] == 5)
            .cloned()
            .collect();
        (!fields.is_empty()).then_some(fields)
    });
    let mut columns = columns;
    columns.sort_by(|a, b| {
        a["sortText"]
            .as_str()
            .unwrap_or_default()
            .cmp(b["sortText"].as_str().unwrap_or_default())
    });
    assert_eq!(
        labels(&columns, 4),
        ["payment_id", "order_id", "payment_method", "amount"]
    );
    assert!(
        columns.iter().all(|c| c["detail"]
            .as_str()
            .is_some_and(|d| d.starts_with("star_star"))),
        "every column is of the relation in scope: {columns:?}"
    );
    lsp.shutdown();
}
