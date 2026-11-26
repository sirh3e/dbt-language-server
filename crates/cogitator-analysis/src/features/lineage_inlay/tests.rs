//! The hints over jaffle_shop and vault_lake models, and over small inputs
//! for what those do not show: a direct column, a rename, an expression, a
//! column read through CTEs, a star, a relation the project does not
//! define, the cap, and the compiled route (no template spans).

use std::path::{Path, PathBuf};

use cogitator_lsp::{PartedInlayHint, Range};
use cogitator_parser::{DocumentParser, JinjaSqlParser};

use super::*;
use crate::artifacts::ArtifactSet;
use crate::features::plan::PlanInput;
use crate::lineage::{typed_column_lineage, ModelLineage};
use crate::DbtContext;

fn project(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata")
        .join(name)
}

fn disk(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

/// The typed lineage of `text` as the model `model` of `ctx`.
fn lineage(ctx: &DbtContext, model: &str, text: &str) -> ModelLineage {
    let parsed = JinjaSqlParser.parse(text, &ctx.dialect).expect("parses");
    let artifacts = ArtifactSet::default();
    let input = PlanInput::from_context(ctx, &parsed.text, Some(model), &artifacts, 0);
    typed_column_lineage(&parsed, &input).expect("lineage")
}

fn label(hint: &PartedInlayHint) -> String {
    hint.label.iter().map(|p| p.value.as_str()).collect()
}

/// `<code before the hint> ⟨label⟩` per hint, trimmed.
fn rendered(text: &str, hints: &[PartedInlayHint]) -> Vec<String> {
    hints
        .iter()
        .map(|h| {
            let line = text.lines().nth(h.position.line as usize).unwrap_or("");
            let code: String = line.chars().take(h.position.character as usize).collect();
            format!("{} ⟨{}⟩", code.trim(), label(h))
        })
        .collect()
}

fn hints(ctx: &DbtContext, model: &str, text: &str, max_len: usize) -> Vec<PartedInlayHint> {
    let lineage = lineage(ctx, model, text);
    let options = LineageInlayOptions {
        enabled: true,
        max_len,
    };
    let read = disk;
    let mut locator = ProjectLocator::new(ctx, &read);
    lineage_inlay_hints(text, &lineage, Range::default(), options, &mut locator)
}

fn jaffle() -> DbtContext {
    crate::build_context(project("jaffle_shop_duckdb").to_str().unwrap())
}

#[test]
fn stg_payments_resolves_through_its_ctes() {
    let ctx = jaffle();
    let path = project("jaffle_shop_duckdb").join("models/staging/stg_payments.sql");
    let text = disk(&path).unwrap();
    let got = rendered(&text, &hints(&ctx, "stg_payments", &text, 80));
    assert_eq!(
        got,
        vec![
            "id as payment_id, ⟨← raw_payments.id⟩",
            "order_id, ⟨← raw_payments.order_id⟩",
            "payment_method, ⟨← raw_payments.payment_method⟩",
            "amount / 100 as amount ⟨← raw_payments.amount · derived⟩",
        ]
    );
}

#[test]
fn customers_shows_several_sources_and_derived_columns() {
    let ctx = jaffle();
    let path = project("jaffle_shop_duckdb").join("models/customers.sql");
    let text = disk(&path).unwrap();
    let got = rendered(&text, &hints(&ctx, "customers", &text, 120));
    let find = |prefix: &str| {
        got.iter()
            .find(|l| l.starts_with(prefix))
            .unwrap_or_else(|| panic!("{prefix} in {got:#?}"))
            .clone()
    };
    assert_eq!(
        find("customers.customer_id"),
        "customers.customer_id, ⟨← stg_customers.customer_id⟩"
    );
    assert_eq!(
        find("customer_payments.total_amount"),
        // without schemas an unqualified `amount` over a join may be either side
        "customer_payments.total_amount as customer_lifetime_value, \
         ⟨← stg_payments.amount, stg_orders.amount · derived⟩"
    );
    assert_eq!(
        find("customer_orders.number_of_orders"),
        "customer_orders.number_of_orders, ⟨← stg_orders.order_id · derived⟩"
    );
}

#[test]
fn a_part_points_at_the_upstream_column() {
    let ctx = jaffle();
    let text = "select payment_id, amount as total from {{ ref('stg_payments') }}\n";
    let found = hints(&ctx, "m", text, 80);
    assert_eq!(
        rendered(text, &found),
        vec![
            "select payment_id ⟨← stg_payments.payment_id⟩",
            "select payment_id, amount as total ⟨← stg_payments.amount⟩",
        ],
        "a rename is no derivation"
    );
    let part = &found[1].label[1];
    let location = part.location.as_ref().expect("a location");
    assert!(
        location.uri.ends_with("stg_payments.sql"),
        "{}",
        location.uri
    );
    let upstream =
        disk(&project("jaffle_shop_duckdb").join("models/staging/stg_payments.sql")).unwrap();
    let line = upstream
        .lines()
        .nth(location.range.start.line as usize)
        .unwrap();
    assert!(line.contains("amount / 100 as amount"), "{line}");
}

#[test]
fn small_shapes() {
    let ctx = jaffle();
    let cases: &[(&str, &[&str])] = &[
        (
            "with c as (select order_id, 1 as one from {{ ref('stg_orders') }})\nselect order_id, one from c\n",
            // a comma with more on its line: the hint stays before it
            &["select order_id ⟨← stg_orders.order_id⟩", "select order_id, one ⟨← derived⟩"],
        ),
        (
            "select o.order_id, p.order_id as p_order\nfrom {{ ref('stg_orders') }} o\njoin {{ ref('stg_payments') }} p on o.order_id = p.order_id\n",
            &[
                "select o.order_id ⟨← stg_orders.order_id⟩",
                "select o.order_id, p.order_id as p_order ⟨← stg_payments.order_id⟩",
            ],
        ),
        (
            "select coalesce(o.order_id, p.order_id) as id\nfrom {{ ref('stg_orders') }} o\njoin {{ ref('stg_payments') }} p on true\n",
            &["select coalesce(o.order_id, p.order_id) as id ⟨← stg_orders.order_id, stg_payments.order_id · derived⟩"],
        ),
        (
            "select a from raw_schema.raw_table\n",
            &["select a ⟨← raw_table.a?⟩"],
        ),
    ];
    for (text, want) in cases {
        let got = rendered(text, &hints(&ctx, "m", text, 120));
        assert_eq!(&got, want, "{text}");
    }
}

#[test]
fn a_star_over_a_model_is_one_hint() {
    let ctx = jaffle();
    let text = "select * from {{ ref('stg_orders') }}\n";
    let got = rendered(text, &hints(&ctx, "m", text, 120));
    assert_eq!(got, vec!["select * ⟨← stg_orders.*⟩"]);
}

#[test]
fn the_cap_cuts_with_an_ellipsis_and_keeps_derived() {
    let ctx = jaffle();
    let text = "select coalesce(o.order_id, p.order_id) as id\nfrom {{ ref('stg_orders') }} o\njoin {{ ref('stg_payments') }} p on true\n";
    let got = hints(&ctx, "m", text, 40);
    assert_eq!(label(&got[0]), "← stg_orders.order_id, … · derived");
    assert!(label(&got[0]).chars().count() <= 40);
    let tooltip = got[0].tooltip.as_deref().unwrap();
    assert!(tooltip.contains("stg_payments.order_id"), "{tooltip}");
    let tiny = hints(&ctx, "m", text, 12);
    assert!(label(&tiny[0]).contains('…'), "{}", label(&tiny[0]));
}

#[test]
fn off_and_compiled_answers_place_nothing() {
    let ctx = jaffle();
    let text = "select order_id from {{ ref('stg_orders') }}\n";
    let mut lineage = lineage(&ctx, "m", text);
    let off = LineageInlayOptions {
        enabled: false,
        max_len: 60,
    };
    assert!(lineage_inlay_hints(text, &lineage, Range::default(), off, &mut ()).is_empty());
    for column in &mut lineage.columns {
        column.end = None; // as the compiled route answers
    }
    let on = LineageInlayOptions::default();
    assert!(lineage_inlay_hints(text, &lineage, Range::default(), on, &mut ()).is_empty());
}

#[test]
fn a_source_part_points_at_its_yaml_column() {
    let ctx = crate::build_context(project("vault_lake").to_str().unwrap());
    let path = project("vault_lake").join("models/raw_vault/hub_item.sql");
    let text = disk(&path).unwrap();
    let got = hints(&ctx, "hub_item", &text, 120);
    let rendered = rendered(&text, &got);
    let at = rendered
        .iter()
        .position(|l| l.ends_with("item_id ⟨← landing.items.item_id⟩"))
        .unwrap_or_else(|| panic!("{rendered:#?}"));
    let location = got[at].label[1].location.as_ref().expect("a location");
    assert!(location.uri.ends_with(".yml"), "{}", location.uri);
}

#[test]
fn vault_lake_models() {
    let ctx = crate::build_context(project("vault_lake").to_str().unwrap());
    let path = project("vault_lake").join("models/dim/dim_items.sql");
    let text = disk(&path).unwrap();
    let got = rendered(&text, &hints(&ctx, "dim_items", &text, 120));
    assert!(
        got.iter()
            .any(|l| l.starts_with("transitions.item_hk || '-'") && l.ends_with("· derived⟩")),
        "{got:#?}"
    );
    assert!(
        got.iter()
            .any(|l| l == "hubs.item_id, ⟨← hub_item.item_id⟩"),
        "the upstream model, not the CTE `hubs`: {got:#?}"
    );
}
