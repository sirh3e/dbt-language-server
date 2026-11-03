//! dbt.relation over the fixture: source tables under the cursor (declared
//! and defaulted coordinates), ref targets, and the file's own model with
//! and without an active profiles target.

use std::path::PathBuf;

use cogitator_analysis::features::{relation_info, TargetRelation};
use cogitator_analysis::{build_context, Documents};
use cogitator_lsp::Position;
use pretty_assertions::assert_eq;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/jaffle_shop_duckdb")
}

fn ctx() -> cogitator_analysis::DbtContext {
    build_context(fixture_root().to_str().unwrap())
}

fn duckdb_target() -> TargetRelation {
    TargetRelation {
        database: Some("jaffle_shop".into()),
        schema: Some("main".into()),
    }
}

/// Open `text` as `model.sql` and resolve the relation at (line, character).
fn at(
    ctx: &cogitator_analysis::DbtContext,
    text: &str,
    character: u32,
    target: Option<&TargetRelation>,
) -> Option<cogitator_analysis::features::RelationInfo> {
    let uri = "file:///tmp/model.sql".to_string();
    let mut docs = Documents::default();
    docs.open(uri.clone(), text, &ctx.dialect);
    relation_info(
        ctx,
        docs.get(&uri),
        &uri,
        Some(Position { line: 0, character }),
        target,
    )
}

#[test]
fn source_table_with_declared_coordinates() {
    let ctx = ctx();
    // cursor on 'orders' inside source('jaffle_shop', 'orders')
    let sql = "select * from {{ source('jaffle_shop', 'orders') }}";
    let info = at(&ctx, sql, 41, Some(&duckdb_target())).expect("source relation");
    assert_eq!(serde_json::to_value(&info).unwrap()["kind"], "source");
    assert_eq!(info.source.as_deref(), Some("jaffle_shop"));
    // declared database/schema beat the target's
    assert_eq!(info.relation, "raw.jaffle_shop.orders");
}

#[test]
fn source_table_defaults_to_target_database_and_source_name_schema() {
    let ctx = ctx();
    // stripe declares no database/schema: database from the target, schema
    // defaults to the source's name (dbt behavior)
    let sql = "select * from {{ source('stripe', 'payments') }}";
    let info = at(&ctx, sql, 36, Some(&duckdb_target())).expect("source relation");
    assert_eq!(info.relation, "jaffle_shop.stripe.payments");
    assert_eq!(info.identifier, "payments");
}

#[test]
fn ref_resolves_the_referenced_model() {
    let ctx = ctx();
    let sql = "select * from {{ ref('customers') }}";
    let info = at(&ctx, sql, 23, Some(&duckdb_target())).expect("model relation");
    assert_eq!(serde_json::to_value(&info).unwrap()["kind"], "model");
    assert_eq!(info.name, "customers");
    assert_eq!(info.source, None);
    assert_eq!(info.relation, "jaffle_shop.main.customers");
}

#[test]
fn file_model_without_target_keeps_nulls() {
    let ctx = ctx();
    let uri = cogitator_lsp::fs_path_to_uri(&fixture_root().join("models/customers.sql"));
    let info = relation_info(&ctx, None, &uri, None, None).expect("model relation");
    assert_eq!(info.database, None);
    assert_eq!(info.schema, None);
    assert_eq!(info.relation, "customers");
}

#[test]
fn unknown_file_yields_none() {
    let ctx = ctx();
    assert!(relation_info(&ctx, None, "file:///tmp/nope.sql", None, None).is_none());
}
