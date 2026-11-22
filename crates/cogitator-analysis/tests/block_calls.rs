//! Macro calls inside statement blocks, nested arguments, filter
//! arguments and `{% call %}` blocks navigate like `{{ m() }}`:
//! references, definition, rename, document links and outgoing calls,
//! over the jaffle fixture.

use std::path::PathBuf;

use cogitator_analysis::{build_context, features, Documents};
use cogitator_lsp::Position;
use rstest::rstest;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/jaffle_shop_duckdb")
}

/// Where `needle` starts in `text`, plus one character.
fn position_of(text: &str, needle: &str) -> Position {
    let at = text.find(needle).unwrap_or_else(|| panic!("{needle}"));
    let line = text[..at].matches('\n').count() as u32;
    let col = (at - text[..at].rfind('\n').map_or(0, |i| i + 1)) as u32;
    Position {
        line,
        character: col + 1,
    }
}

#[rstest]
#[case::set("{% set x = full_name('a', 'b') %}\nselect 1")]
#[case::if_block("{% if full_name('a', 'b') %}select 1{% endif %}")]
#[case::for_block("{% for c in full_name(ref('customers'), 'b') %}{{ c }}{% endfor %}")]
#[case::do_block("{% do full_name('a', 'b') %}")]
#[case::nested("select {{ times_five(full_name('a', 'b')) }}")]
#[case::filter_argument("select {{ x | default(full_name('a', 'b')) }}")]
#[case::call_block("{% call full_name('a', 'b') %}select 1{% endcall %}")]
#[case::qualified_set("{% set x = jaffle_shop.full_name('a', 'b') %}")]
fn a_call_in_a_block_navigates(#[case] text: &str) {
    let ctx = build_context(root().to_str().unwrap());
    let mut docs = Documents::default();
    let uri = cogitator_lsp::fs_path_to_uri(&root().join("models/probe_blocks.sql"));
    docs.open(uri.clone(), text, &ctx.dialect);
    let doc = docs.get(&uri).unwrap();
    let pos = position_of(text, "full_name(");

    let refs = features::references_at(&ctx, Some(doc), &uri, pos, false);
    let files: Vec<&str> = refs
        .iter()
        .map(|l| l.uri.rsplit('/').next().unwrap())
        .collect();
    // the call resolves to the macro: its other callers come back (an
    // unsaved document is in no graph; e2e covers calls on disk)
    assert!(files.contains(&"customers.sql"), "{text}: {refs:?}");

    let def = features::definition(&ctx, doc, &uri, pos);
    assert!(def.uri.ends_with("jaffle_macros.sql"), "{text}: {def:?}");

    let prepared = features::prepare_rename_at(&ctx, doc, &uri, pos).expect("not refused");
    assert!(prepared.is_some(), "{text}");

    let tips: Vec<String> = features::document_links_at(&ctx, doc, &uri)
        .into_iter()
        .filter_map(|l| l.tooltip)
        .collect();
    assert!(
        tips.contains(&"macro jaffle_shop.full_name".to_string()),
        "{text}: {tips:?}"
    );
}

#[test]
fn outgoing_calls_reach_into_blocks() {
    let ctx = build_context(root().to_str().unwrap());
    let text = concat!(
        "{% macro outer(cols) %}{% set x = full_name('a', 'b') %}",
        "{% for c in cols.split(',') %}{% do times_five(c) %}{% endfor %}",
        "{{ return(range(3)) }}{% endmacro %}",
    );
    let path = root().join("macros/probe_blocks.sql");
    let item = cogitator_lsp::CallHierarchyItem {
        name: "outer".into(),
        kind: cogitator_lsp::symbol_kind::FUNCTION,
        uri: cogitator_lsp::fs_path_to_uri(&path),
        range: Default::default(),
        selection_range: Default::default(),
    };
    let calls = features::outgoing_in_text(&ctx, &item, &path, text);
    let names: Vec<&str> = calls.iter().map(|c| c.to.name.as_str()).collect();
    assert_eq!(names, vec!["full_name", "times_five"]);
}

#[test]
fn context_functions_in_blocks_get_no_links() {
    let ctx = build_context(root().to_str().unwrap());
    let mut docs = Documents::default();
    let text = concat!(
        "{% set d = {'a': 1} %}{% for k, v in d.items() %}{% endfor %}",
        "{% for i in range(3) %}{% endfor %}{% do log('x') %}",
        "{% if x is defined %}{% endif %}{{ x | default(1) }}",
    );
    let uri = cogitator_lsp::fs_path_to_uri(&root().join("models/probe_blocks.sql"));
    docs.open(uri.clone(), text, &ctx.dialect);
    let links = features::document_links_at(&ctx, docs.get(&uri).unwrap(), &uri);
    assert!(links.is_empty(), "{links:?}");
}
