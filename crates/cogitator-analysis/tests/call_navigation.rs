//! Definition and hover from anywhere in a call: the word `ref`, a
//! project name, the parenthesis, and a macro named inside a hook's
//! string, all go where the name they are about is defined.

use std::path::PathBuf;

use cogitator_analysis::{build_context, features, Documents};
use cogitator_lsp::Position;
use rstest::rstest;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/jaffle_shop_duckdb")
}

const TEXT: &str = "{{ config(post_hook=\"{{ times_five(1) }}\") }}\n\
select * from {{ ref('stg_orders') }}\n\
join {{ source('jaffle_shop', 'orders') }} using (id)\n\
where {{ full_name('a', 'b') }} is not null\n";

/// The position of the `nth` occurrence of `needle` in TEXT, plus `plus`.
fn at(needle: &str, plus: u32) -> Position {
    let offset = TEXT
        .find(needle)
        .unwrap_or_else(|| panic!("{needle} in TEXT"));
    let line = TEXT[..offset].matches('\n').count() as u32;
    let col = offset - TEXT[..offset].rfind('\n').map(|p| p + 1).unwrap_or(0);
    Position {
        line,
        character: col as u32 + plus,
    }
}

fn with_doc<T>(
    f: impl FnOnce(&cogitator_analysis::DbtContext, &cogitator_analysis::Document, &str) -> T,
) -> T {
    let ctx = build_context(fixture_root().to_str().unwrap());
    let uri = cogitator_lsp::fs_path_to_uri(&fixture_root().join("models/probe_calls.sql"));
    let mut docs = Documents::default();
    docs.open(uri.clone(), TEXT, &ctx.dialect);
    let doc = docs.get(&uri).unwrap();
    f(&ctx, doc, &uri)
}

#[rstest]
#[case::the_name("'stg_orders'", 2, "stg_orders.sql")]
#[case::the_word_ref("ref('stg", 1, "stg_orders.sql")]
#[case::the_parenthesis("ref('stg", 3, "stg_orders.sql")]
#[case::the_word_source("source(", 2, "schema.yml")]
#[case::a_macro("full_name(", 2, "jaffle_macros.sql")]
#[case::a_macro_in_a_hook("times_five", 3, "jaffle_macros.sql")]
fn definition_from_anywhere_in_the_call(
    #[case] needle: &str,
    #[case] plus: u32,
    #[case] file: &str,
) {
    with_doc(|ctx, doc, uri| {
        let location = features::definition(ctx, doc, uri, at(needle, plus));
        assert!(
            location.uri.ends_with(file),
            "{needle}+{plus}: wanted {file}, got {location:?}"
        );
    });
}

#[test]
fn a_plain_word_is_still_itself() {
    with_doc(|ctx, doc, uri| {
        // `select` is no call: the answer stays where the cursor is
        let location = features::definition(ctx, doc, uri, at("select", 2));
        assert!(location.uri.ends_with("probe_calls.sql"), "{location:?}");
    });
}

#[rstest]
#[case::on_the_word_ref("ref('stg", 1, "**model** `stg_orders`", "stg_orders.sql:1")]
#[case::on_a_macro("full_name(", 2, "**macro** `full_name`", "jaffle_macros.sql:1")]
#[case::in_a_hook("times_five", 3, "**macro** `times_five`", "jaffle_macros.sql:7")]
fn hover_names_it_and_links_its_definition(
    #[case] needle: &str,
    #[case] plus: u32,
    #[case] head: &str,
    #[case] link: &str,
) {
    with_doc(|ctx, doc, _| {
        let hover = features::hover(ctx, doc, at(needle, plus));
        assert!(hover.starts_with(head), "{needle}: {hover}");
        assert!(
            hover.contains(&format!("Definition: [{link}](file://")),
            "{needle}: {hover}"
        );
    });
}
