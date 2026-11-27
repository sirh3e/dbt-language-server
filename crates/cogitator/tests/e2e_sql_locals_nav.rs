//! Go to definition, references and highlights on the SQL names of one
//! model, end to end on the jaffle fixture: a table alias in two scopes,
//! a CTE and a column alias read through it and in ORDER BY — and the
//! definition of `ref()`, which still goes to the model.

mod harness;
use harness::jaffle;
use serde_json::{json, Value};

const PROBE: &str = "\
with paid as (
    select order_id, amount as total from {{ ref('stg_payments') }}
)
select o.order_id, p.total as paid_total
from {{ ref('stg_orders') }} o
join paid p on p.order_id = o.order_id
where exists (select 1 from paid o where o.total > 0)
order by total
";

/// The position `into` bytes into the `nth` (from 0) `needle` of `text`.
fn at(text: &str, needle: &str, nth: usize, into: usize) -> Value {
    let offset = text.match_indices(needle).nth(nth).unwrap().0 + into;
    let before = &text[..offset];
    let line = before.matches('\n').count();
    let character = offset - before.rfind('\n').map_or(0, |i| i + 1);
    json!({"line": line, "character": character})
}

fn params(uri: &str, position: Value) -> Value {
    json!({"textDocument": {"uri": uri}, "position": position})
}

fn booted_with_probe() -> (harness::fixture::TestProject, harness::client::Lsp, String) {
    let project = jaffle();
    let path = project.root.join("models/nav_probe.sql");
    std::fs::write(&path, PROBE).unwrap();
    let (mut lsp, _) = project.booted();
    let uri = cogitator_lsp::fs_path_to_uri(&path);
    lsp.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": uri, "languageId": "sql", "version": 1, "text": PROBE}}),
    );
    (project, lsp, uri)
}

#[test]
fn definition_goes_to_the_declaration_in_its_scope() {
    let (_project, mut lsp, uri) = booted_with_probe();
    for (from, to) in [
        // the outer alias `o`
        (at(PROBE, "o.order_id", 1, 0), at(PROBE, "}} o", 0, 3)),
        // the subquery's alias of the same name
        (at(PROBE, "o.total", 0, 0), at(PROBE, "paid o", 0, 5)),
        // a CTE from its use
        (at(PROBE, "paid p", 0, 1), at(PROBE, "paid as", 0, 0)),
        // a column alias through the CTE, and read by ORDER BY
        (at(PROBE, "p.total", 0, 3), at(PROBE, "total from", 0, 0)),
        (at(PROBE, "by total", 0, 4), at(PROBE, "total from", 0, 0)),
    ] {
        let got = lsp.request("textDocument/definition", params(&uri, from.clone()));
        assert_eq!(got["result"]["uri"], json!(uri), "{from} → {got}");
        assert_eq!(got["result"]["range"]["start"], to, "{from} → {got}");
    }
    // ref() still goes to the model's file
    let got = lsp.request(
        "textDocument/definition",
        params(&uri, at(PROBE, "stg_orders", 0, 2)),
    );
    let target = got["result"]["uri"].as_str().unwrap();
    assert!(target.ends_with("stg_orders.sql"), "{got}");
}

#[test]
fn references_and_highlights_list_every_use() {
    let (_project, mut lsp, uri) = booted_with_probe();
    let refs = |lsp: &mut harness::client::Lsp, pos: Value, decl: bool| -> usize {
        let mut p = params(&uri, pos);
        p["context"] = json!({"includeDeclaration": decl});
        let got = lsp.request("textDocument/references", p);
        got["result"]
            .as_array()
            .unwrap_or_else(|| panic!("{got}"))
            .len()
    };
    // the CTE: its name, `join paid` and the subquery's `from paid`
    assert_eq!(refs(&mut lsp, at(PROBE, "paid as", 0, 0), true), 3);
    // the outer alias: its name and two qualified columns, not the inner `o`
    assert_eq!(refs(&mut lsp, at(PROBE, "}} o", 0, 3), true), 3);
    assert_eq!(refs(&mut lsp, at(PROBE, "}} o", 0, 3), false), 2);
    // the column alias: `as total`, `p.total`, `o.total` and ORDER BY
    assert_eq!(refs(&mut lsp, at(PROBE, "total from", 0, 0), true), 4);

    let got = lsp.request(
        "textDocument/documentHighlight",
        params(&uri, at(PROBE, "p.total", 0, 3)),
    );
    let kinds: Vec<i64> = got["result"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["kind"].as_i64().unwrap())
        .collect();
    assert_eq!(kinds, vec![3, 2, 2, 2], "{got}");
}
