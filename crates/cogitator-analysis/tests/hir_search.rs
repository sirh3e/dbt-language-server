//! dbt.search's structural queries over the jaffle_shop fixture (read
//! in place): matches like ast-grep's, and the models that could not be
//! searched named with why.

use std::path::{Path, PathBuf};

use cogitator_analysis::build_context;
use cogitator_analysis::features::hir_search::search;

fn jaffle() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/jaffle_shop_duckdb")
}

#[test]
fn queries_over_the_project() {
    let ctx = build_context(jaffle().to_str().unwrap());
    let read = |p: &Path| std::fs::read_to_string(p).ok();

    let report = search(
        &ctx,
        "reads stg_customers and select-star from model",
        None,
        &read,
    )
    .expect("a query");
    assert!(
        report.models.contains(&"customers".to_string()),
        "{report:#?}"
    );
    let clauses: Vec<&str> = report
        .matches
        .iter()
        .filter(|m| m.env["model"] == "customers")
        .map(|m| m.env["clause"].as_str())
        .collect();
    assert!(clauses.contains(&"reads stg_customers"), "{clauses:?}");
    assert!(clauses.contains(&"select-star from model"), "{clauses:?}");
    assert!(report.matches.iter().all(|m| m.uri.ends_with(".sql")));
    // orders.sql loops over payment methods: not searched, and said so
    assert!(
        report
            .not_searched
            .iter()
            .any(|n| n.model == "orders" && n.why.contains("control blocks")),
        "{:?}",
        report.not_searched
    );
    assert!(report.searched > 0);

    let none = search(&ctx, "reads source:nowhere", None, &read).unwrap();
    assert!(none.matches.is_empty() && none.models.is_empty());
    let one = search(&ctx, "reads model", Some("customers"), &read).unwrap();
    assert_eq!(one.models, ["customers"]);

    let bad = search(&ctx, "find everything", None, &read).unwrap_err();
    assert!(bad.to_message().contains("cannot read the query"));
}

#[test]
fn or_not_and_parentheses() {
    let ctx = build_context(jaffle().to_str().unwrap());
    let read = |p: &Path| std::fs::read_to_string(p).ok();
    let models = |query: &str| search(&ctx, query, None, &read).expect(query).models;

    // `or`: either model's readers
    let either = models("reads raw_customers or reads raw_orders");
    assert!(either.contains(&"stg_customers".to_string()), "{either:?}");
    assert!(either.contains(&"stg_orders".to_string()), "{either:?}");
    assert!(!either.contains(&"stg_payments".to_string()), "{either:?}");

    // `and not`: readers of stg_orders, minus those that also read stg_customers
    let without = models("reads stg_orders and not reads stg_customers");
    assert!(
        without.contains(&"customer_orders_v1".to_string()),
        "{without:?}"
    );
    assert!(!without.contains(&"customers".to_string()), "{without:?}");

    // parentheses change the grouping
    let grouped = models("(reads raw_customers or reads raw_orders) and not reads raw_orders");
    assert_eq!(grouped, ["stg_customers"]);

    // only the clauses that made a model match show their sites: an `or`
    // branch that failed does not, nor does anything under `not`
    let report = search(
        &ctx,
        "reads raw_payments or reads raw_customers",
        Some("stg_customers"),
        &read,
    )
    .unwrap();
    let clauses: Vec<&str> = report
        .matches
        .iter()
        .map(|m| m.env["clause"].as_str())
        .collect();
    assert_eq!(clauses, ["reads raw_customers"]);

    // a model the query holds in only through `not` is listed at the top
    // of its file, naming the whole query
    let report = search(&ctx, "not reads model", Some("stg_stripe_payments"), &read).unwrap();
    assert_eq!(report.models, ["stg_stripe_payments"]);
    let only = &report.matches[..];
    assert_eq!(only.len(), 1, "{only:#?}");
    assert_eq!(only[0].env["clause"], "not reads model");
    assert_eq!(only[0].range, Default::default());
}
