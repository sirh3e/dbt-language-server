//! The grammar-aware completion engine: context classification and the
//! resulting suggestions per cursor position.

use std::path::PathBuf;

use cogitator_analysis::{build_context, features, Documents};
use cogitator_lsp::{completion_kind, CompletionItem, Position};
use pretty_assertions::assert_eq;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/jaffle_shop_duckdb")
}

fn complete(text: &str, line: u32, character: u32) -> Vec<CompletionItem> {
    let ctx = build_context(fixture_root().to_str().unwrap());
    let uri = "file:///tmp/engine.sql";
    let mut docs = Documents::default();
    docs.open(uri.to_string(), text, &ctx.dialect);
    features::completion(&ctx, docs.get(uri).unwrap(), Position { line, character })
}

const DOC: &str = "with recent as (\n    select customer_id, order_total from {{ ref('customers') }} c\n)\nselect \nfrom \n";

#[test]
fn relation_position_suggests_ctes_and_jinja_snippets() {
    // cursor after `from ` on line 4
    let items = complete(DOC, 4, 5);
    let cte = items
        .iter()
        .find(|i| i.label == "recent")
        .expect("cte relation");
    assert_eq!(cte.insert_text, "recent");
    let model = items
        .iter()
        .find(|i| i.label == "customers")
        .expect("model relation");
    assert_eq!(model.insert_text, "{{ ref('customers') }}");
    // CTEs (tier a) rank before models (tier c)
    assert!(cte.sort_text < model.sort_text);
    // no raw keyword flood in relation position
    assert!(!items.iter().any(|i| i.label == "qualify"));
}

#[test]
fn member_access_resolves_alias_to_model_columns() {
    // `c.` where c aliases {{ ref('customers') }}
    let text = "select c. from {{ ref('customers') }} c";
    let items = complete(text, 0, 9);
    assert!(!items.is_empty());
    let column = items
        .iter()
        .find(|i| i.label == "customer_id")
        .expect("column via alias");
    assert_eq!(column.kind, completion_kind::FIELD);
    assert_eq!(column.detail, "Column of customers");
    assert!(column.documentation.contains("unique identifier"));
    // context-exact tier: nothing outranks the qualifier's columns
    assert!(items.iter().all(|i| i.sort_text.starts_with('0')));
}

#[test]
fn member_access_resolves_cte_columns() {
    let items = complete(DOC, 3, 7); // after `select ` — first check column position…
    assert!(items.iter().any(|i| i.detail == "Column of customers"));

    // …then `recent.` member access resolves the CTE's select list
    let text = format!("{DOC}select recent. from recent\n");
    let items = complete(&text, 5, 14);
    let col = items
        .iter()
        .find(|i| i.label == "customer_id")
        .expect("cte column");
    assert_eq!(col.detail, "Column of recent (CTE)");
}

#[test]
fn keyword_continuations() {
    let by = complete("select 1 from t group ", 0, 22);
    assert_eq!(by.first().map(|i| i.label.as_str()), Some("by"));
    assert!(by[0].sort_text.starts_with('0'));

    let join = complete("select 1 from t left ", 0, 21);
    assert_eq!(join.first().map(|i| i.label.as_str()), Some("join"));

    let start = complete("", 0, 0);
    assert_eq!(start.first().map(|i| i.label.as_str()), Some("select"));
}

#[test]
fn column_position_ranks_columns_before_keywords() {
    // after `where ` with a ref in scope
    let text = "select 1 from {{ ref('customers') }} c where ";
    let items = complete(text, 0, 46);
    let column_rank = items
        .iter()
        .find(|i| i.label == "customer_id")
        .map(|i| i.sort_text.clone())
        .expect("column in scope");
    let keyword_rank = items
        .iter()
        .find(|i| i.label == "qualify")
        .map(|i| i.sort_text.clone())
        .expect("keywords still available");
    assert!(column_rank < keyword_rank, "{column_rank} < {keyword_rank}");
}

#[test]
fn jinja_triggers_unchanged() {
    let text = "select * from {{ ref('";
    let items = complete(text, 0, text.len() as u32);
    let customers = items
        .iter()
        .find(|i| i.label == "customers")
        .expect("ref trigger");
    assert_eq!(customers.insert_text, "customers') }}");
}
