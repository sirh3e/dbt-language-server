//! The opt-in lint checks (03 §10) over the fixture context: each one
//! on, off, and exempt where the rules say so.

use std::path::PathBuf;

use cogitator_analysis::features::lint::{lint, LintOptions};
use cogitator_analysis::{build_context, Documents};

fn ctx() -> cogitator_analysis::DbtContext {
    build_context(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/jaffle_shop_duckdb")
            .to_str()
            .unwrap(),
    )
}

fn doc<'a>(
    docs: &'a mut Documents,
    ctx: &cogitator_analysis::DbtContext,
    uri: &str,
    text: &str,
) -> &'a cogitator_analysis::Document {
    docs.open(uri.to_string(), text, &ctx.dialect);
    docs.get(uri).unwrap()
}

#[test]
fn select_star_warns_outside_staging_with_a_column_quickfix() {
    let ctx = ctx();
    let mut docs = Documents::default();
    let uri = "file:///p/models/mart_probe.sql";
    let d = doc(
        &mut docs,
        &ctx,
        uri,
        "select * from {{ ref('stg_customers') }}",
    );
    let opts = LintOptions {
        select_star: true,
        ..LintOptions::default()
    };
    let items = lint(&ctx, d, uri, opts, None);
    assert_eq!(items.len(), 1, "{items:?}");
    assert_eq!(items[0].code, "lint-select-star");
    let data = items[0].data.as_ref().expect("column quickfix");
    assert!(data.suggestions[0].contains("customer_id"), "{data:?}");

    // staging models are exempt, and the check honors its flag
    let stg_uri = "file:///p/models/staging/stg_probe.sql";
    let s = doc(
        &mut docs,
        &ctx,
        stg_uri,
        "select * from {{ ref('stg_customers') }}",
    );
    assert!(lint(&ctx, s, stg_uri, opts, None).is_empty());
    let d2 = docs.get(uri).unwrap();
    assert!(lint(&ctx, d2, uri, LintOptions::default(), None).is_empty());
}

#[test]
fn unaliased_columns_flag_only_multi_relation_queries() {
    let ctx = ctx();
    let mut docs = Documents::default();
    let uri = "file:///p/models/join_probe.sql";
    let text = "select customer_id, o.total from {{ ref('stg_customers') }} c \
                join {{ ref('stg_orders') }} o on c.customer_id = o.customer_id";
    let d = doc(&mut docs, &ctx, uri, text);
    let opts = LintOptions {
        unaliased_columns: true,
        ..LintOptions::default()
    };
    let items = lint(&ctx, d, uri, opts, None);
    assert_eq!(items.len(), 1, "{items:?}");
    assert!(
        items[0].message.contains("'customer_id'"),
        "{}",
        items[0].message
    );

    // one relation: bare columns are unambiguous
    let single_uri = "file:///p/models/single_probe.sql";
    let s = doc(
        &mut docs,
        &ctx,
        single_uri,
        "select customer_id from {{ ref('stg_customers') }}",
    );
    assert!(lint(&ctx, s, single_uri, opts, None).is_empty());
}

#[test]
fn trailing_comma_is_dialect_gated() {
    let mut docs = Documents::default();
    let uri = "file:///p/models/comma_probe.sql";
    let text = "select customer_id, from {{ ref('stg_customers') }}";

    // the fixture's duckdb dialect tolerates it → quiet
    let ctx = ctx();
    let d = doc(&mut docs, &ctx, uri, text);
    let opts = LintOptions {
        trailing_comma: true,
        ..LintOptions::default()
    };
    assert!(lint(&ctx, d, uri, opts, None).is_empty());

    // an unresolved dialect does not — the comma is flagged
    let strict = cogitator_analysis::DbtContext::default();
    let mut strict_docs = Documents::default();
    let s = doc(&mut strict_docs, &strict, uri, text);
    let items = lint(&strict, s, uri, opts, None);
    assert_eq!(items.len(), 1, "{items:?}");
    assert_eq!(items[0].code, "lint-trailing-comma");
}

#[test]
fn undocumented_columns_hint_from_lineage() {
    let ctx = ctx();
    let mut docs = Documents::default();
    let model = ctx.model_detail_map.get("customers").expect("customers");
    let uri = cogitator_lsp::fs_path_to_uri(&model.uri);
    let text = "select customer_id, 1 as brand_new_col from {{ ref('stg_customers') }}";
    let d = doc(&mut docs, &ctx, &uri, text);
    let lineage =
        cogitator_analysis::lineage::column_lineage(&d.parsed, &ctx.dialect).expect("lineage");
    let opts = LintOptions {
        undocumented_columns: true,
        ..LintOptions::default()
    };
    let items = lint(&ctx, d, &uri, opts, Some(&lineage));
    assert_eq!(items.len(), 1, "{items:?}");
    assert!(
        items[0].message.contains("brand_new_col"),
        "{}",
        items[0].message
    );
    assert!(
        !items[0].message.contains("customer_id"),
        "{}",
        items[0].message
    );
}
