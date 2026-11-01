//! Scan-cache behavior (05 §6.2 fast path) and the column-lineage engine.

use std::path::PathBuf;

use cogitator_analysis::lineage::{column_lineage, SourceRef};
use cogitator_analysis::ContextBuilder;
use cogitator_dialects::Dialect;
use cogitator_parser::{DocumentParser, JinjaSqlParser, ParsedDocument};
use pretty_assertions::assert_eq;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/jaffle_shop_duckdb")
}

#[test]
fn second_refresh_is_served_from_cache() {
    let root = fixture_root();
    let mut builder = ContextBuilder::default();

    let first = builder.build(root.to_str().unwrap());
    let (_, misses_after_first) = builder.cache_stats();
    assert!(misses_after_first > 0, "cold build parses files");

    let second = builder.build(root.to_str().unwrap());
    let (hits, misses_after_second) = builder.cache_stats();
    assert_eq!(
        misses_after_second, misses_after_first,
        "warm build re-parses nothing"
    );
    assert!(hits >= misses_after_first, "warm build hits for every file");

    // identical results from the cached path
    let mut a: Vec<_> = first.model_detail_map.keys().collect();
    let mut b: Vec<_> = second.model_detail_map.keys().collect();
    a.sort();
    b.sort();
    assert_eq!(a, b);
    assert_eq!(first.graph.sorted_edges(), second.graph.sorted_edges());
}

fn parse(text: &str) -> ParsedDocument {
    JinjaSqlParser.parse(text, &Dialect::new("duckdb")).unwrap()
}

#[test]
fn lineage_direct_ref_columns() {
    let doc = parse("select customer_id, amount as total from {{ ref('stg_payments') }}");
    let lineage = column_lineage(&doc, &Dialect::new("duckdb")).expect("lineage");
    let names: Vec<_> = lineage.columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["customer_id", "total"]);
    assert_eq!(
        lineage.columns[1].sources,
        vec![SourceRef::Model {
            name: "stg_payments".into(),
            column: "amount".into()
        }]
    );
}

#[test]
fn lineage_resolves_through_ctes_and_joins() {
    let doc = parse(concat!(
        "with orders as (\n",
        "    select order_id, customer_id from {{ ref('stg_orders') }}\n",
        "),\n",
        "payments as (\n",
        "    select order_id, amount from {{ source('stripe', 'payment') }}\n",
        ")\n",
        "select o.customer_id, p.amount\n",
        "from orders o join payments p on o.order_id = p.order_id\n",
    ));
    let lineage = column_lineage(&doc, &Dialect::new("duckdb")).expect("lineage");
    let by_name = |n: &str| {
        lineage
            .columns
            .iter()
            .find(|c| c.name == n)
            .unwrap_or_else(|| panic!("column {n}"))
    };
    assert_eq!(
        by_name("customer_id").sources,
        vec![SourceRef::Model {
            name: "stg_orders".into(),
            column: "customer_id".into()
        }]
    );
    assert_eq!(
        by_name("amount").sources,
        vec![SourceRef::Source {
            source: "stripe".into(),
            table: "payment".into(),
            column: "amount".into()
        }]
    );
}

#[test]
fn lineage_wildcard_and_expr_alias() {
    let doc = parse(concat!(
        "select *, first_name || ' ' || last_name as full_name\n",
        "from {{ ref('stg_customers') }}\n",
    ));
    let lineage = column_lineage(&doc, &Dialect::new("duckdb")).expect("lineage");
    let star = lineage
        .columns
        .iter()
        .find(|c| c.name == "*")
        .expect("wildcard");
    assert_eq!(
        star.sources,
        vec![SourceRef::Model {
            name: "stg_customers".into(),
            column: "*".into()
        }]
    );
    let full = lineage
        .columns
        .iter()
        .find(|c| c.name == "full_name")
        .expect("alias");
    assert!(full.sources.contains(&SourceRef::Model {
        name: "stg_customers".into(),
        column: "first_name".into()
    }));
    assert!(full.sources.contains(&SourceRef::Model {
        name: "stg_customers".into(),
        column: "last_name".into()
    }));
}

#[test]
fn lineage_declines_on_control_blocks() {
    let doc = parse("select 1 {% if flag %}, 2{% endif %} from {{ ref('a') }}");
    assert!(column_lineage(&doc, &Dialect::new("duckdb")).is_none());
}

#[test]
fn lineage_on_the_real_customers_model() {
    let path = fixture_root().join("models/customers.sql");
    let text = std::fs::read_to_string(path).unwrap();
    let doc = parse(&text);
    let lineage = column_lineage(&doc, &Dialect::new("duckdb")).expect("customers lineage");
    assert!(!lineage.columns.is_empty());
    // `select * from final` resolves through the CTE chain to staging models
    let star = lineage
        .columns
        .iter()
        .find(|c| c.name == "*")
        .expect("star column");
    assert!(
        star.sources.iter().any(|s| matches!(
            s,
            SourceRef::Model { name, column } if name.starts_with("stg_") && column == "customer_id"
        )),
        "resolved through CTEs to staging columns: {:?}",
        star.sources
    );
}
