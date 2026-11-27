//! workspace/symbol's plain query and its prefixes, and documentSymbol's
//! ref()/source() calls, over jaffle_shop.

use std::path::{Path, PathBuf};

use cogitator_analysis::{build_context, features, selectors, Documents};
use cogitator_lsp::{symbol_kind, SymbolInformation};
use pretty_assertions::assert_eq;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/jaffle_shop_duckdb")
}

fn ctx() -> cogitator_analysis::DbtContext {
    build_context(fixture_root().to_str().unwrap())
}

fn find<'a>(symbols: &'a [SymbolInformation], name: &str) -> Option<&'a SymbolInformation> {
    symbols.iter().find(|s| s.name == name)
}

/// (query, name, kind, container, file the symbol is in)
#[test]
fn plain_queries_reach_every_part() {
    let ctx = ctx();
    let cases: &[(&str, &str, i32, Option<&str>, &str)] = &[
        (
            "customers",
            "customers",
            symbol_kind::CLASS,
            None,
            "customers.sql",
        ),
        (
            "jaffle_shop",
            "jaffle_shop",
            symbol_kind::INTERFACE,
            None,
            "schema.yml",
        ),
        (
            "jaffle_shop.or",
            "jaffle_shop.orders",
            symbol_kind::INTERFACE,
            Some("jaffle_shop"),
            "schema.yml",
        ),
        (
            "customer_id",
            "customers.customer_id",
            symbol_kind::FIELD,
            Some("customers"),
            "schema.yml",
        ),
        (
            "gift_card",
            "orders.gift_card_amount",
            symbol_kind::FIELD,
            Some("orders"),
            "schema.yml",
        ),
        (
            "not_null on orders",
            "not_null on orders.order_id",
            symbol_kind::METHOD,
            Some("orders"),
            "schema.yml",
        ),
        (
            "order_count",
            "not_null on customer_orders.order_count (v1)",
            symbol_kind::METHOD,
            Some("customer_orders"),
            "customer_orders.yml",
        ),
    ];
    for (query, name, kind, container, file) in cases {
        let symbols = features::workspace_symbols(&ctx, query);
        let s =
            find(&symbols, name).unwrap_or_else(|| panic!("{query}: no {name} in {symbols:#?}"));
        assert_eq!(
            (s.kind, s.container_name.as_deref()),
            (*kind, *container),
            "{query}: {name}"
        );
        assert!(
            s.location.uri.ends_with(file),
            "{query}: {}",
            s.location.uri
        );
    }
}

#[test]
fn a_column_is_at_its_name() {
    let ctx = ctx();
    let symbols = features::workspace_symbols(&ctx, "customers.customer_id");
    let column = find(&symbols, "customers.customer_id").expect("the column");
    let range = column.location.range;
    assert_eq!(
        (range.start.line, range.start.character, range.end.character),
        (8, 14, 14 + "customer_id".len() as u32)
    );
}

#[test]
fn an_empty_query_lists_definitions_alone() {
    let ctx = ctx();
    let all = features::workspace_symbols(&ctx, "");
    for kind in [symbol_kind::FIELD, symbol_kind::METHOD] {
        assert!(
            all.iter().all(|s| s.kind != kind),
            "kind {kind} in an empty query"
        );
    }
    for name in ["customers", "jaffle_shop", "stripe.payments", "full_name"] {
        assert!(find(&all, name).is_some(), "{name} missing");
    }
    let mut names: Vec<_> = all.iter().map(|s| s.name.clone()).collect();
    let listed = names.clone();
    names.sort();
    assert_eq!(listed, names, "sorted by name");
}

/// (query, the only kind it answers)
#[test]
fn prefixes_keep_to_one_kind() {
    let ctx = ctx();
    let cases = [
        ("column:order_id", symbol_kind::FIELD),
        ("column:", symbol_kind::FIELD),
        ("test:unique", symbol_kind::METHOD),
        ("test:", symbol_kind::METHOD),
        ("selector:", symbol_kind::NAMESPACE),
    ];
    for (query, kind) in cases {
        let symbols = features::workspace_symbols(&ctx, query);
        assert!(!symbols.is_empty(), "{query}");
        assert!(
            symbols.iter().all(|s| s.kind == kind),
            "{query}: {symbols:#?}"
        );
    }
    let ids = features::workspace_symbols(&ctx, "column:order_id");
    assert!(ids.iter().all(|s| s.name.ends_with("order_id")), "{ids:#?}");
}

#[test]
fn unit_tests_come_by_name() {
    let ctx = ctx();
    let schema = fixture_root().join("models/schema.yml");
    let unit = "\nunit_tests:\n  - name: orders_add_up\n    model: orders\n";
    let read = |p: &Path| {
        let text = std::fs::read_to_string(p).ok()?;
        Some(if p.ends_with("models/schema.yml") {
            text + unit
        } else {
            text
        })
    };
    let universe = selectors::universe(&ctx, &|_| None);
    let symbols = features::workspace_symbols_with(&ctx, &universe, "add_up", &read);
    let s = find(&symbols, "orders_add_up").expect("the unit test");
    assert_eq!(
        (s.kind, s.container_name.as_deref()),
        (symbol_kind::METHOD, Some("orders"))
    );
    assert!(cogitator_lsp::uri_to_fs_path(&s.location.uri)
        .is_some_and(|p| Path::new(&p).ends_with(schema.strip_prefix(fixture_root()).unwrap())));
}

#[test]
fn document_symbols_list_calls_with_ctes_by_position() {
    let ctx = ctx();
    let mut docs = Documents::default();
    let text = "with o as (\n  select * from {{ ref('orders') }}\n),\n\
                p as (select * from {{ source('stripe', 'payments') }})\nselect * from o\n";
    docs.open("file:///d.sql".into(), text, &ctx.dialect);
    let symbols = features::document_symbols(docs.get("file:///d.sql").unwrap(), "file:///d.sql");
    let listed: Vec<_> = symbols.iter().map(|s| (s.name.as_str(), s.kind)).collect();
    assert_eq!(
        listed,
        vec![
            ("o", symbol_kind::STRUCT),
            ("ref(orders)", symbol_kind::PACKAGE),
            ("p", symbol_kind::STRUCT),
            ("source(stripe, payments)", symbol_kind::PACKAGE),
        ]
    );
}
