//! Column completion per cursor position (`|` marks the cursor): what is
//! offered, and in what order (the items sorted by `sortText`, as every
//! client sorts them before its own filtering).

use std::path::PathBuf;

use cogitator_analysis::{build_context, features, DbtContext, Documents};
use cogitator_lsp::{completion_kind, CompletionItem, Position};
use pretty_assertions::assert_eq;
use rstest::rstest;

static CTX: std::sync::LazyLock<DbtContext> = std::sync::LazyLock::new(|| {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/jaffle_shop_duckdb");
    build_context(root.to_str().unwrap())
});

fn ctx() -> &'static DbtContext {
    &CTX
}

/// The items at the `|`, sorted as a client sorts them.
fn complete(ctx: &DbtContext, marked: &str) -> Vec<CompletionItem> {
    let at = marked.find('|').expect("cursor mark");
    let text = marked.replacen('|', "", 1);
    let line = text[..at].matches('\n').count() as u32;
    let character = (at - text[..at].rfind('\n').map_or(0, |i| i + 1)) as u32;
    let uri = "file:///tmp/columns.sql";
    let mut docs = Documents::default();
    docs.open(uri.to_string(), &text, &ctx.dialect);
    let read = |p: &std::path::Path| std::fs::read_to_string(p).ok();
    let mut items = features::completion_with(
        ctx,
        docs.get(uri).unwrap(),
        Position { line, character },
        &read,
    );
    items.sort_by(|a, b| a.sort_text.cmp(&b.sort_text));
    items
}

fn labels(items: &[CompletionItem], n: usize) -> Vec<&str> {
    items.iter().take(n).map(|i| i.label.as_str()).collect()
}

/// Every column item before the first item that is not a column.
fn columns_first(items: &[CompletionItem]) {
    let first_other = items
        .iter()
        .position(|i| i.kind != completion_kind::FIELD)
        .unwrap_or(items.len());
    assert!(
        items[first_other..]
            .iter()
            .all(|i| i.kind != completion_kind::FIELD || !i.sort_text.starts_with('0')),
        "a column in scope ranks below a global: {:?}",
        labels(items, first_other + 3)
    );
}

const JOIN: &str =
    "from {{ ref('orders') }} o join {{ ref('customers') }} c on o.customer_id = c.customer_id";

#[rstest]
#[case::bare_alias(&format!("select o.| {JOIN}"), &["order_id", "customer_id", "order_date"])]
#[case::typing(&format!("select o.cu| {JOIN}"), &["order_id", "customer_id", "order_date"])]
#[case::second_alias(&format!("select c.| {JOIN}"), &["customer_id", "first_name", "last_name"])]
#[case::where_(&format!("select 1 {JOIN} where c.|"), &["customer_id", "first_name", "last_name"])]
#[case::unaliased("select customers.| from {{ ref('customers') }}", &["customer_id", "first_name", "last_name"])]
#[case::cte_star(
    "with recent as (select * from {{ ref('customers') }}) select r.| from recent r",
    &["customer_id", "first_name", "last_name"],
)]
#[case::cte_list(
    "with recent as (select customer_id, number_of_orders as n from {{ ref('customers') }}) select recent.| from recent",
    &["customer_id", "n"],
)]
#[case::incomplete(&format!("select o.| {JOIN} where"), &["order_id", "customer_id", "order_date"])]
fn after_a_qualifier_only_its_relations_columns(#[case] marked: &str, #[case] first: &[&str]) {
    let items = complete(ctx(), marked);
    assert_eq!(labels(&items, first.len()), first, "{marked}");
    assert!(
        items.iter().all(|i| i.kind == completion_kind::FIELD),
        "{marked}"
    );
    assert!(items[0].preselect);
}

#[test]
fn a_qualified_column_says_its_relation_and_description() {
    let items = complete(ctx(), &format!("select c.| {JOIN}"));
    let id = items.iter().find(|i| i.label == "customer_id").unwrap();
    assert_eq!(id.detail, "customers");
    assert!(
        id.documentation.contains("unique identifier"),
        "{}",
        id.documentation
    );
}

#[rstest]
#[case::select_list(&format!("select | {JOIN}"))]
#[case::select_list_more(&format!("select o.order_id, | {JOIN}"))]
#[case::where_(&format!("select 1 {JOIN} where |"))]
#[case::group_by(&format!("select 1 {JOIN} group by |"))]
#[case::order_by(&format!("select 1 {JOIN} order by |"))]
#[case::qualify(&format!("select 1 {JOIN} qualify |"))]
#[case::join_on("select 1 from {{ ref('orders') }} o join {{ ref('customers') }} c on |")]
#[case::incomplete(&format!("select | {JOIN} where"))]
fn a_column_position_ranks_the_ambiguous_qualified_first(#[case] marked: &str) {
    let items = complete(ctx(), marked);
    // customer_id is both relations': qualified, then the unambiguous
    assert_eq!(
        labels(&items, 4),
        [
            "c.customer_id",
            "o.customer_id",
            "amount",
            "bank_transfer_amount"
        ],
        "{marked}"
    );
    assert!(items[0].preselect, "{marked}");
    columns_first(&items);
    let qualified = items.iter().find(|i| i.label == "o.amount").unwrap();
    assert_eq!(qualified.filter_text.as_deref(), Some("amount"));
    assert_eq!(qualified.insert_text, "o.amount");
    let bare = items.iter().position(|i| i.label == "amount").unwrap();
    let alias = items.iter().position(|i| i.label == "o.amount").unwrap();
    assert!(bare < alias, "an unambiguous column: bare first");
    let ambiguous = items.iter().find(|i| i.label == "customer_id").unwrap();
    assert_eq!(ambiguous.detail, "ambiguous: o, c");
}

#[test]
fn a_single_relation_offers_bare_then_qualified_before_globals() {
    let items = complete(ctx(), "select | from {{ ref('customers') }} c");
    assert_eq!(labels(&items, 2), ["customer_id", "first_name"]);
    let qualified = items
        .iter()
        .position(|i| i.label == "c.customer_id")
        .unwrap();
    let keyword = items.iter().position(|i| i.label == "select").unwrap();
    assert!(qualified < keyword);
    columns_first(&items);
}

#[rstest]
#[case::from("with recent as (select 1 as n) select * from |")]
#[case::join("with recent as (select 1 as n) select * from recent r join |")]
#[case::typing("with recent as (select 1 as n) select * from rec|")]
fn after_from_ctes_then_refs_then_sources(#[case] marked: &str) {
    let items = complete(ctx(), marked);
    assert_eq!(items[0].label, "recent", "{marked}");
    let first_ref = items
        .iter()
        .position(|i| i.insert_text.starts_with("{{ ref("))
        .unwrap();
    let first_source = items
        .iter()
        .position(|i| i.insert_text.starts_with("{{ source("))
        .unwrap();
    let last_ref = items
        .iter()
        .rposition(|i| i.insert_text.starts_with("{{ ref("))
        .unwrap();
    assert!(0 < first_ref && last_ref < first_source, "{marked}");
    assert!(!items.iter().any(|i| i.kind == completion_kind::FIELD));
}

/// A model whose YAML documents no columns: its column lineage, read
/// through `read`.
#[test]
fn an_undocumented_model_is_asked_its_lineage() {
    let root = std::env::temp_dir().join(format!("cogitator-columns-{}", std::process::id()));
    std::fs::create_dir_all(root.join("models")).unwrap();
    std::fs::write(
        root.join("dbt_project.yml"),
        "name: lin\nversion: '1'\nconfig-version: 2\nprofile: lin\nmodel-paths: [models]\n",
    )
    .unwrap();
    std::fs::write(
        root.join("models/totals.sql"),
        "select customer_id, sum(amount) as total from raw_payments group by 1",
    )
    .unwrap();
    let ctx = build_context(root.to_str().unwrap());
    let items = complete(&ctx, "select t.| from {{ ref('totals') }} t");
    assert_eq!(labels(&items, 2), ["customer_id", "total"]);
    assert_eq!(items[0].detail, "totals");
    std::fs::remove_dir_all(root).ok();
}
