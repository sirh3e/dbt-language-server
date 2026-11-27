//! The column graph over a real project's lineage (testdata/jaffle_shop_duckdb):
//! each model's untyped lineage, built into one graph, answers across models.

use std::path::PathBuf;

use cogitator_analysis::lineage::{column_lineage, ColumnGraph, ModelLineage, Relation};
use cogitator_analysis::{build_context, DbtContext};
use pretty_assertions::assert_eq;

fn project() -> (DbtContext, Vec<(String, ModelLineage)>) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/jaffle_shop_duckdb");
    let ctx = build_context(root.to_str().unwrap());
    let mut lineages: Vec<(String, ModelLineage)> = ctx
        .model_detail_map
        .iter()
        .filter_map(|(name, model)| {
            let text = std::fs::read_to_string(&model.uri).ok()?;
            let parsed = cogitator_parser::pipeline::parse_document(&text, &ctx.dialect).ok()?;
            Some((name.clone(), column_lineage(&parsed, &ctx.dialect)?))
        })
        .collect();
    lineages.sort_by(|a, b| a.0.cmp(&b.0));
    (ctx, lineages)
}

fn graph() -> ColumnGraph {
    let (_, lineages) = project();
    ColumnGraph::build(lineages.iter().map(|(m, l)| (m.as_str(), l)))
}

fn label(g: &ColumnGraph, node: cogitator_analysis::lineage::graph::NodeIndex) -> String {
    let c = g.column(node);
    match &c.relation {
        Relation::Model(m) => format!("{m}.{}", c.column),
        Relation::Source { source, table } => format!("{source}.{table}.{}", c.column),
        Relation::Unknown(n) => format!("?{n}.{}", c.column),
    }
}

#[test]
fn a_staging_column_is_read_by_the_marts() {
    let g = graph();
    let start = g
        .find_model_column("stg_customers", "customer_id")
        .expect("the column");
    let reached: Vec<String> = g
        .downstream(&[start], None)
        .iter()
        .map(|r| label(&g, r.node))
        .collect();
    assert!(
        reached.iter().any(|l| l == "customers.customer_id"),
        "{reached:?}"
    );
    assert!(
        reached.iter().all(|l| !l.starts_with("stg_customers.")),
        "{reached:?}"
    );
}

#[test]
fn a_mart_column_comes_from_its_staging_column() {
    let g = graph();
    let start = g
        .find_model_column("customers", "customer_id")
        .expect("the column");
    let chain: Vec<String> = g
        .origin_chain(start, 64)
        .iter()
        .map(|&n| label(&g, n))
        .collect();
    assert_eq!(
        chain.first().map(String::as_str),
        Some("customers.customer_id")
    );
    assert!(
        chain.iter().any(|l| l == "stg_customers.customer_id"),
        "{chain:?}"
    );
}

#[test]
fn every_model_with_lineage_is_in_the_graph() {
    let (_, lineages) = project();
    let g = ColumnGraph::build(lineages.iter().map(|(m, l)| (m.as_str(), l)));
    let models: Vec<&str> = g.models().collect();
    let want: Vec<&str> = lineages.iter().map(|(m, _)| m.as_str()).collect();
    assert_eq!(models, want);
    for (model, lineage) in &lineages {
        assert!(g.outputs(model).len() >= lineage.columns.len(), "{model}");
    }
}
