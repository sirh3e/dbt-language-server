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
