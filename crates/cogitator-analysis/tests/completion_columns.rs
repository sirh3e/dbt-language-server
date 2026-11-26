//! Column completion with the data type and description beside each
//! item: YAML types, catalog types, inferred CTE columns with inherited
//! descriptions, `doc()` resolved, and the plain fallback shape.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use cogitator_analysis::artifacts::catalog::{CatalogColumn, CatalogIndex, CatalogTable};
use cogitator_analysis::{build_context, features, DbtContext, Documents};
use cogitator_lsp::{adapt, CompletionItem, CompletionSupport, Position};

fn root(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata")
        .join(name)
}

fn complete(
    ctx: &DbtContext,
    text: &str,
    line: u32,
    character: u32,
    hints: &features::ModelHints,
) -> Vec<CompletionItem> {
    let uri = "file:///tmp/columns.sql";
    let mut docs = Documents::default();
    docs.open(uri.to_string(), text, &ctx.dialect);
    features::completion_with(
        ctx,
        docs.get(uri).unwrap(),
        Position { line, character },
        hints,
    )
}

fn find<'a>(items: &'a [CompletionItem], label: &str) -> &'a CompletionItem {
    items
        .iter()
        .find(|i| i.label == label)
        .unwrap_or_else(|| panic!("{label}: {items:#?}"))
}

fn resolve(ctx: &DbtContext, item: &CompletionItem, hints: &features::ModelHints) -> String {
    let data = item.data.as_ref().expect("lazy data");
    let (docs, markdown) =
        features::resolve_completion_documentation_with(ctx, data, hints).expect("resolves");
    assert!(markdown);
    docs
}

#[test]
fn a_model_column_shows_its_yaml_type_and_relation() {
    let ctx = build_context(root("type_check").to_str().unwrap());
    let hints = features::ModelHints::default();
    let items = complete(
        &ctx,
        "select o. from {{ ref('stg_orders') }} o",
        0,
        9,
        &hints,
    );
    let status = find(&items, "status");
    let details = status.label_details.as_ref().expect("labelDetails");
    assert_eq!(details.detail.as_deref(), Some(" varchar"));
    assert_eq!(details.description.as_deref(), Some("stg_orders"));
    let docs = resolve(&ctx, status, &hints);
    assert!(
        docs.starts_with("`status` · `varchar` (YAML data_type)\n\nColumn of stg_orders"),
        "{docs}"
    );
    // without labelDetails support: the type and relation in `detail`
    let plain = adapt(status.clone(), CompletionSupport::default());
    assert_eq!(plain.detail, "varchar · stg_orders");
    assert!(plain.label_details.is_none());
}

#[test]
fn a_source_column_takes_the_catalog_type() {
    let ctx = build_context(root("type_check").to_str().unwrap());
    let mut sources = HashMap::new();
    sources.insert(
        ("shop".to_string(), "orders".to_string()),
        CatalogTable {
            columns: vec![CatalogColumn {
                name: "amount".into(),
                data_type: "DECIMAL(10,2)".into(),
                comment: None,
                index: Some(2),
            }],
            ..Default::default()
        },
    );
    let hints = features::ModelHints {
        catalog: Some(Arc::new(CatalogIndex {
            generated_at: None,
            sources,
            models: HashMap::new(),
        })),
        ..Default::default()
    };
    let items = complete(
        &ctx,
        "select s. from {{ source('shop', 'orders') }} s",
        0,
        9,
        &hints,
    );
    let amount = find(&items, "amount");
    assert_eq!(
        amount
            .label_details
            .as_ref()
            .and_then(|d| d.detail.as_deref()),
        Some(" decimal(10,2)")
    );
    assert!(resolve(&ctx, amount, &hints).contains("(from catalog)"));
}

#[test]
fn a_cte_column_inherits_its_description_eagerly() {
    let ctx = build_context(root("jaffle_shop_duckdb").to_str().unwrap());
    let hints = features::ModelHints::default();
    let text = "with o as (select order_id, order_date from {{ ref('orders') }})\nselect o. from o";
    let items = complete(&ctx, text, 1, 9, &hints);
    let day = find(&items, "order_date");
    assert_eq!(
        day.label_details
            .as_ref()
            .and_then(|d| d.description.as_deref()),
        Some("o (CTE)")
    );
    assert!(day.markdown);
    assert!(day.data.is_none(), "a CTE's docs are filled with the item");
    assert!(
        day.documentation
            .contains("Date (UTC) that the order was placed"),
        "{}",
        day.documentation
    );
    assert!(
        day.documentation
            .contains("*Description from orders.order_date*"),
        "{}",
        day.documentation
    );
}

#[test]
fn a_doc_block_description_is_resolved() {
    let ctx = build_context(root("jaffle_shop_duckdb").to_str().unwrap());
    let hints = features::ModelHints::default();
    let items = complete(&ctx, "select o. from {{ ref('orders') }} o", 0, 9, &hints);
    let docs = resolve(&ctx, find(&items, "status"), &hints);
    assert!(
        docs.contains("Orders can be one of the following statuses"),
        "{docs}"
    );
}

#[test]
fn models_and_macros_say_what_they_are_beside_the_label() {
    let ctx = build_context(root("jaffle_shop_duckdb").to_str().unwrap());
    let hints = features::ModelHints::default();
    let refs = complete(&ctx, "select * from {{ ref('", 0, 22, &hints);
    let model = find(&refs, "customers");
    assert_eq!(
        model
            .label_details
            .as_ref()
            .and_then(|d| d.description.as_deref()),
        Some("model")
    );
    let macros = complete(&ctx, "select {{ ", 0, 10, &hints);
    let times_five = find(&macros, "times_five");
    let details = times_five.label_details.as_ref().expect("macro details");
    assert!(
        details
            .detail
            .as_deref()
            .is_some_and(|d| d.starts_with('(')),
        "{details:?}"
    );
}
