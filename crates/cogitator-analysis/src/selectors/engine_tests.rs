//! The engine on the jaffle fixture, end to end through the context.

use super::*;

fn jaffle() -> (crate::DbtContext, Universe) {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/jaffle_shop_duckdb");
    let ctx = crate::build_context(root.to_str().unwrap());
    let read = |p: &std::path::Path| std::fs::read_to_string(p).ok();
    let u = universe(&ctx, &read);
    (ctx, u)
}

/// Selected node names, tests left out to keep the table short.
fn picked(u: &Universe, ctx: &crate::DbtContext, text: &str) -> Vec<String> {
    let e = select(u, &ctx.selectors.definitions, text, Indirect::Empty).unwrap();
    sorted(u, &e)
        .iter()
        .filter(|n| n.resource_type != ResourceType::Test || text.contains("test"))
        .map(|n| n.display_name())
        .collect()
}

#[test]
fn methods_on_the_fixture() {
    let (ctx, u) = jaffle();
    let cases: &[(&str, &[&str])] = &[
        ("tag:nightly", &["customers"]),
        ("customers", &["customers"]),
        (
            "+customers",
            &[
                "customers",
                "stg_customer_status",
                "stg_customers",
                "stg_orders",
                "stg_payments",
                "raw_customers",
                "raw_orders",
                "raw_payments",
            ],
        ),
        ("staging", &[]),
        (
            "jaffle_shop.staging",
            &[
                "stg_customers",
                "stg_orders",
                "stg_payments",
                "stg_stripe_payments",
            ],
        ),
        (
            "models/staging",
            &[
                "stg_customers",
                "stg_orders",
                "stg_payments",
                "stg_stripe_payments",
            ],
        ),
        (
            "config.materialized:view,package:this",
            &[
                "stg_customers",
                "stg_orders",
                "stg_payments",
                "stg_stripe_payments",
            ],
        ),
        (
            "config.materialized:table",
            &[
                "customer_orders.v1",
                "customer_orders.v2",
                "customers",
                "orders",
            ],
        ),
        ("package:jaffle_package", &["stg_customer_status"]),
        (
            "resource_type:seed",
            &["raw_customers", "raw_orders", "raw_payments"],
        ),
        ("resource_type:snapshot", &["orders_snapshot"]),
        (
            "source:stripe+",
            &["stg_stripe_payments", "stripe.payments"],
        ),
        ("source:jaffle_shop.orders", &["jaffle_shop.orders"]),
        (
            "customer_orders",
            &["customer_orders.v1", "customer_orders.v2"],
        ),
        ("customer_orders.v1", &["customer_orders.v1"]),
        ("version:latest", &["customer_orders.v2"]),
        ("version:old", &["customer_orders.v1"]),
        ("file:orders.sql", &["orders"]),
        ("test_type:singular", &["assert_positive_order_amount"]),
        (
            "test_name:relationships",
            &["relationships_orders_customer_id__customer_id__ref_customers_"],
        ),
        ("resource_type:analysis", &["order_counts"]),
        ("orders+1,resource_type:analysis", &[]),
        ("customers+1,resource_type:analysis", &["order_counts"]),
    ];
    for (text, expected) in cases {
        assert_eq!(picked(&u, &ctx, text), *expected, "{text}");
    }
}

#[test]
fn selectors_yml_definitions_evaluate() {
    let (ctx, u) = jaffle();
    let nightly = select_definition(&u, &ctx.selectors.definitions, "nightly");
    let names: Vec<String> = sorted(&u, &nightly)
        .iter()
        .map(|n| n.display_name())
        .collect();
    // eager indirect selection brings customers' tests along, as dbt ls does
    assert_eq!(names[0], "customers");
    assert!(names.contains(&"unique_customers_customer_id".to_string()));
    assert!(nightly.issues.is_empty());
    let marts = select_definition(&u, &ctx.selectors.definitions, "marts_only");
    assert!(sorted(&u, &marts).iter().any(|n| n.name == "customers"));
}

#[test]
fn issues_are_reported_in_words() {
    let (ctx, u) = jaffle();
    let e = select(
        &u,
        &ctx.selectors.definitions,
        "state:modified selector:nope",
        Indirect::Eager,
    )
    .unwrap();
    let words: Vec<String> = e.issues.iter().map(issue_message).collect();
    assert_eq!(
        words,
        vec![
            "`state:` needs a previous run's artifacts (dbt --state); it selects nothing here"
                .to_string(),
            "selectors.yml defines no selector `nope`".to_string(),
        ]
    );
}
