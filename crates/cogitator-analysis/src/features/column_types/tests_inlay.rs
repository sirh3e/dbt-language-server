//! `inlay.types` over `testdata/type_check`.

use cogitator_lsp::Range;
use cogitator_parser::{DocumentParser, JinjaSqlParser};

use super::tests::project;
use super::{column_types, type_inlay_hints};

fn hints(model: &str, text: &str) -> Vec<(u32, u32, String)> {
    let (ctx, snapshot) = project();
    let types = column_types(&ctx, &snapshot, model).expect("types");
    let doc = JinjaSqlParser.parse(text, &ctx.dialect).expect("parse");
    type_inlay_hints(&doc, &ctx.dialect, &types, Range::default())
        .into_iter()
        .map(|h| (h.position.line, h.position.character, h.label))
        .collect()
}

#[test]
fn each_typed_output_gets_its_type_after_the_item() {
    let text = std::fs::read_to_string(super::tests::fixture_root().join("models/stg_orders.sql"))
        .expect("model");
    let found = hints("stg_orders", &text);
    let labels: Vec<&str> = found.iter().map(|(_, _, l)| l.as_str()).collect();
    assert!(labels.contains(&": bigint"), "{found:?}");
    assert!(labels.contains(&": date"), "{found:?}");
    // the hint sits right after `as order_id` on its line
    let line = text.lines().nth(1).expect("line");
    let (l, c, _) = found
        .iter()
        .find(|(_, _, l)| l == ": bigint")
        .expect("bigint");
    assert_eq!((*l, *c as usize), (1, line.len() - 1));
}

#[test]
fn an_unproven_type_is_marked_and_a_union_has_none() {
    let found = hints("order_stats", "select o.order_id, count(*) as order_count, c.code\nfrom {{ ref('stg_orders') }} as o left join {{ ref('legacy_codes') }} as c on o.order_id = c.code\ngroup by o.order_id, c.code");
    assert!(
        found.iter().any(|(_, _, l)| l.starts_with(": ≈")),
        "{found:?}"
    );
    let union = hints("unioned", "select 1 as n\nunion all\nselect 2.5 as n");
    assert!(union.is_empty(), "{union:?}");
}
