//! prepareRename and the macro call hierarchy over the fixture.

use std::path::PathBuf;

use cogitator_analysis::{build_context, features, Documents};
use cogitator_lsp::Position;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/jaffle_shop_duckdb")
}

#[test]
fn prepare_rename_accepts_refs_only() {
    let ctx = build_context(fixture_root().to_str().unwrap());
    let mut docs = Documents::default();
    let uri = "file:///p/models/prep.sql";
    docs.open(
        uri.into(),
        "select amount from {{ ref('customers') }}",
        &ctx.dialect,
    );
    let doc = docs.get(uri).unwrap();

    let on_ref = features::prepare_rename(
        doc,
        Position {
            line: 0,
            character: 28,
        },
    )
    .expect("ref is renameable");
    assert_eq!(on_ref.placeholder, "customers");
    assert_eq!(on_ref.range.start.line, 0);
    assert!(on_ref.range.start.character < on_ref.range.end.character);

    // a plain column is not
    let on_column = features::prepare_rename(
        doc,
        Position {
            line: 0,
            character: 8,
        },
    );
    assert!(on_column.is_none(), "{on_column:?}");
}

#[test]
fn macro_call_hierarchy_finds_callers_with_ranges() {
    let ctx = build_context(fixture_root().to_str().unwrap());
    let mut docs = Documents::default();
    let uri = "file:///p/models/ch.sql";
    docs.open(
        uri.into(),
        "select {{ full_name('a', 'b') }} as n",
        &ctx.dialect,
    );
    let item = features::prepare_macro_hierarchy(
        &ctx,
        docs.get(uri).unwrap(),
        Position {
            line: 0,
            character: 12,
        },
    )
    .expect("macro item");
    assert_eq!(item.name, "full_name");
    assert_eq!(item.kind, cogitator_lsp::symbol_kind::FUNCTION);
    assert!(item.uri.ends_with("jaffle_macros.sql"), "{}", item.uri);

    let calls = features::macro_incoming_calls(&ctx, &item);
    let callers: Vec<&str> = calls.iter().map(|c| c.from.name.as_str()).collect();
    assert!(callers.contains(&"customers"), "{callers:?}");
    let customers = calls.iter().find(|c| c.from.name == "customers").unwrap();
    assert!(!customers.from_ranges.is_empty());
    assert!(customers.from.uri.ends_with("customers.sql"));

    // an unknown macro prepares nothing
    docs.open(
        "file:///p/models/none.sql".into(),
        "select {{ nope_macro() }}",
        &ctx.dialect,
    );
    let none = features::prepare_macro_hierarchy(
        &ctx,
        docs.get("file:///p/models/none.sql").unwrap(),
        Position {
            line: 0,
            character: 12,
        },
    );
    assert!(none.is_none());
}
