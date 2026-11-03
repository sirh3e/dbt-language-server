//! Tag and selector search plus the dbt.projectGraph payload.

use std::path::PathBuf;

use cogitator_analysis::util::config_tags;
use cogitator_analysis::{build_context, features};
use pretty_assertions::assert_eq;

fn ctx() -> cogitator_analysis::DbtContext {
    build_context(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/jaffle_shop_duckdb")
            .to_str()
            .unwrap(),
    )
}

#[test]
fn yaml_tags_reach_model_details() {
    let ctx = ctx();
    let customers = ctx.model_detail_map.get("customers").unwrap();
    assert_eq!(
        customers.tags,
        vec!["mart".to_string(), "nightly".to_string()]
    );
    assert!(ctx.model_detail_map.get("orders").unwrap().tags.is_empty());
}

#[test]
fn sql_config_tags_extract() {
    assert_eq!(
        config_tags("{{ config(materialized='table', tags=['nightly', \"pii\"]) }} select 1"),
        vec!["nightly".to_string(), "pii".to_string()]
    );
    assert_eq!(
        config_tags("{{ config(tags='hourly') }}"),
        vec!["hourly".to_string()]
    );
    assert!(config_tags("select 1 -- tags=['nope']").is_empty());
}

#[test]
fn selectors_yml_is_loaded() {
    let ctx = ctx();
    let nightly = ctx.selector_detail_map.get("nightly").expect("selector");
    assert_eq!(nightly.description, "Everything the nightly job builds");
    assert!(nightly.uri.ends_with("selectors.yml"));
    assert!(nightly.range.start.line > 0);
    assert!(ctx.selector_detail_map.contains_key("marts_only"));
}

#[test]
fn workspace_symbol_tag_and_selector_queries() {
    let ctx = ctx();
    let tagged = features::workspace_symbols(&ctx, "tag:nightly");
    assert_eq!(
        tagged.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
        vec!["customers"]
    );
    // bare prefix lists every tagged model
    assert!(!features::workspace_symbols(&ctx, "tag:").is_empty());

    let selectors = features::workspace_symbols(&ctx, "selector:marts");
    assert_eq!(selectors[0].name, "marts_only");
    assert!(selectors[0].location.uri.ends_with("selectors.yml"));

    // plain queries also find selectors by name
    assert!(features::workspace_symbols(&ctx, "nightly")
        .iter()
        .any(|s| s.name == "nightly"));
}

#[test]
fn project_graph_layers_columns_edges() {
    let ctx = ctx();
    let graph = serde_json::to_value(features::project_graph_json(&ctx)).unwrap();
    let nodes = graph["nodes"].as_array().unwrap();
    let by_id = |id: &str| nodes.iter().find(|n| n["id"] == id).unwrap();

    let stg = by_id("model:stg_customers");
    let customers = by_id("model:customers");
    assert!(
        customers["layer"].as_u64() > stg["layer"].as_u64(),
        "dependents sit right of dependencies: {graph}"
    );
    assert_eq!(customers["tags"], serde_json::json!(["mart", "nightly"]));
    assert!(
        customers["columns"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["name"] == "customer_id"),
        "columns ride along for the DAG boxes"
    );
    assert!(customers["uri"].is_string());

    let edges = graph["edges"].as_array().unwrap();
    assert!(
        edges
            .iter()
            .any(|e| e["from"] == "model:stg_customers" && e["to"] == "model:customers"),
        "{edges:?}"
    );
}
