//! Snapshot, singular-test and analysis scanning over the fixture:
//! snapshots are ref-able citizens of the model map, tests/analyses
//! join the graph as leaf dependents, and none of them is ever a
//! dead model or a dangling ref target.

use std::path::PathBuf;

use cogitator_analysis::context::ModelKind;
use cogitator_analysis::{build_context, features, Documents};

fn ctx() -> cogitator_analysis::DbtContext {
    build_context(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/jaffle_shop_duckdb")
            .to_str()
            .unwrap(),
    )
}

#[test]
fn snapshots_are_refable_models() {
    let ctx = ctx();
    let snap = ctx
        .model_detail_map
        .get("orders_snapshot")
        .expect("snapshot scanned by block name");
    assert_eq!(snap.kind, ModelKind::Snapshot);
    assert_eq!(snap.description, "Snapshot File");
    assert!(
        snap.uri.ends_with("snapshots/orders_snapshot.sql"),
        "{:?}",
        snap.uri
    );

    // a ref() to the snapshot resolves — no dangling diagnostic
    let mut docs = Documents::default();
    let uri = "file:///p/models/uses_snap.sql";
    docs.open(
        uri.into(),
        "select * from {{ ref('orders_snapshot') }}",
        &ctx.dialect,
    );
    let diags = features::dangling_ref_diagnostics(&ctx, docs.get(uri).unwrap());
    assert!(diags.is_empty(), "{diags:?}");

    // definition on the ref jumps into the snapshot file
    let location = features::definition(
        &ctx,
        docs.get(uri).unwrap(),
        uri,
        cogitator_lsp::Position {
            line: 0,
            character: 24,
        },
    );
    assert!(
        location.uri.ends_with("orders_snapshot.sql"),
        "{location:?}"
    );
}

#[test]
fn seeds_and_snapshots_never_dangle_in_the_graph() {
    let ctx = ctx();
    let dangling: Vec<&str> = ctx
        .graph
        .dangling_refs()
        .map(|(_, to)| to.name.as_str())
        .collect();
    assert!(
        !dangling.iter().any(|n| n.starts_with("raw_")),
        "seed refs resolved via the model map: {dangling:?}"
    );
    assert!(!dangling.contains(&"orders_snapshot"), "{dangling:?}");
}

#[test]
fn tests_and_analyses_join_the_graph_as_leaves() {
    let ctx = ctx();
    let (nodes, _) = ctx.graph.dag();
    let find = |name: &str| nodes.iter().find(|(n, _)| n.name == name).map(|(n, _)| *n);

    let test = find("assert_positive_order_amount").expect("singular test node");
    assert_eq!(test.kind.as_str(), "test");
    assert!(test.uri.ends_with("tests/assert_positive_order_amount.sql"));

    let analysis = find("order_counts").expect("analysis node");
    assert_eq!(analysis.kind.as_str(), "analysis");

    // the test shows up among the model's dependents (find-references)
    assert!(
        ctx.graph
            .downstream_of("orders")
            .any(|n| n.name == "assert_positive_order_amount"),
        "test depends on orders"
    );

    // …but never in the dead-model report, and it keeps nothing alive
    // that was not alive before (the fixture dead set is asserted
    // exactly in tests/dead_models.rs)
    assert!(!ctx
        .model_detail_map
        .contains_key("assert_positive_order_amount"));
    assert!(!ctx.model_detail_map.contains_key("order_counts"));
}
