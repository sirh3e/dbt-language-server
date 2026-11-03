//! Dead-model detection over the fixture: graph downstream, exposure
//! references, selector mentions, seed and package exclusions.

use std::path::PathBuf;

use cogitator_analysis::{build_context, features};

#[test]
fn fixture_dead_set_is_exactly_the_unreferenced_marts() {
    let ctx = build_context(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/jaffle_shop_duckdb")
            .to_str()
            .unwrap(),
    );
    let dead: Vec<String> = features::dead_models(&ctx)
        .into_iter()
        .map(|d| d.name)
        .collect();
    // customers: named by the marts_only selector → alive
    // orders: customer_orders depends on it → alive
    // raw_*: seeds → never flagged; stg_customer_status: package model
    assert_eq!(
        dead,
        vec![
            "customer_orders".to_string(),
            "customer_orders_v1".to_string(),
            "stg_stripe_payments".to_string(),
        ],
        "fixture dead set"
    );
}
