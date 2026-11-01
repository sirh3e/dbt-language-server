//! Tests for the post-parity features (FEATURE_REQUESTS.md P1–P3).

use std::path::PathBuf;

use magos::{build_context, features, yaml_features, Documents};
use pretty_assertions::assert_eq;
use vox::Position;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/jaffle_shop_duckdb")
}

fn ctx() -> magos::DbtContext {
    build_context(fixture_root().to_str().unwrap())
}

fn open_inline<'a>(
    docs: &'a mut Documents,
    ctx: &magos::DbtContext,
    uri: &str,
    text: &str,
) -> &'a magos::Document {
    docs.open(uri.to_string(), text, &ctx.dialect);
    docs.get(uri).unwrap()
}

#[test]
fn did_close_forgets_document() {
    let ctx = ctx();
    let mut docs = Documents::default();
    docs.open("file:///a.sql".into(), "select 1", &ctx.dialect);
    assert!(docs.get("file:///a.sql").is_some());
    docs.close("file:///a.sql");
    assert!(docs.get("file:///a.sql").is_none());
}

#[test]
fn references_finds_dependents_via_graph() {
    let ctx = ctx();
    // stg_orders is referenced by at least customers/orders
    let refs = features::references(
        &ctx,
        None,
        "file:///w/models/staging/stg_orders.sql",
        Position::default(),
    );
    assert!(!refs.is_empty(), "stg_orders has dependents");
    assert!(refs.iter().all(|l| l.uri.starts_with("file://")));
    assert!(refs.iter().any(|l| l.uri.ends_with(".sql")));
}

#[test]
fn dangling_refs_flagged_known_refs_clean() {
    let ctx = ctx();
    let mut docs = Documents::default();
    let doc = open_inline(
        &mut docs,
        &ctx,
        "file:///x.sql",
        "select * from {{ ref('stg_orders') }}\njoin {{ ref('does_not_exist') }}\n",
    );
    let diags = features::dangling_ref_diagnostics(&ctx, doc);
    assert_eq!(diags.len(), 1);
    assert!(diags[0].message.contains("does_not_exist"));
    assert_eq!(diags[0].range.start.line, 1);
    assert_eq!(diags[0].code, "dangling-ref");
}

#[test]
fn document_symbols_lists_ctes_and_macros() {
    let ctx = ctx();
    let mut docs = Documents::default();
    let text = "with orders as (\n select 1\n)\nselect * from orders\n";
    let doc = open_inline(&mut docs, &ctx, "file:///s.sql", text);
    let symbols = features::document_symbols(doc, "file:///s.sql");
    assert!(symbols.iter().any(|s| s.name == "orders"));
}

#[test]
fn document_highlights_same_literal() {
    let ctx = ctx();
    let mut docs = Documents::default();
    let text = "with orders as (select 1)\nselect * from orders join orders using (id)";
    let doc = open_inline(&mut docs, &ctx, "file:///h.sql", text);
    let col = text.lines().nth(1).unwrap().find("orders").unwrap() as u32;
    let highlights = features::document_highlights(
        doc,
        Position {
            line: 1,
            character: col + 1,
        },
    );
    assert!(
        highlights.len() >= 3,
        "all three occurrences: {highlights:?}"
    );
}

#[test]
fn folding_ranges_cover_multiline_parens() {
    let ctx = ctx();
    let mut docs = Documents::default();
    let text = "with orders as (\n select 1\n)\nselect * from orders";
    let doc = open_inline(&mut docs, &ctx, "file:///f.sql", text);
    let ranges = features::folding_ranges(doc);
    assert!(
        ranges.iter().any(|r| r.start_line == 0 && r.end_line == 2),
        "{ranges:?}"
    );
}

#[test]
fn semantic_tokens_delta_encoding_is_consistent() {
    let ctx = ctx();
    let mut docs = Documents::default();
    let text = "select * from {{ ref('stg_orders') }} where x = 1";
    let doc = open_inline(&mut docs, &ctx, "file:///t.sql", text);
    let tokens = features::semantic_tokens(doc);
    assert_eq!(tokens.data.len() % 5, 0);
    assert!(!tokens.data.is_empty());
    // decode and confirm one token covers stg_orders as "variable"
    let variable_idx = vox::SEMANTIC_TOKEN_TYPES
        .iter()
        .position(|t| *t == "variable")
        .unwrap() as u32;
    let (mut line, mut col) = (0u32, 0u32);
    let mut found = false;
    for chunk in tokens.data.chunks(5) {
        line += chunk[0];
        col = if chunk[0] == 0 {
            col + chunk[1]
        } else {
            chunk[1]
        };
        if chunk[3] == variable_idx {
            let start = col as usize;
            assert_eq!(&text[start..start + chunk[2] as usize], "stg_orders");
            found = true;
        }
    }
    assert!(found, "stg_orders encoded as variable");
    let _ = line;
}

#[test]
fn signature_help_for_known_macro() {
    let ctx = ctx();
    let macro_name = ctx
        .macro_detail_map
        .get("jaffle_shop")
        .and_then(|m| m.keys().next().cloned())
        .expect("fixture has macros");
    let mut docs = Documents::default();
    let text = format!("select {{{{ {macro_name}(col, ");
    let doc = open_inline(&mut docs, &ctx, "file:///sig.sql", &text);
    let help = features::signature_help(
        &ctx,
        doc,
        Position {
            line: 0,
            character: text.len() as u32,
        },
    );
    let help = help.expect("signature help for macro call");
    assert!(help.signatures[0].label.starts_with(&macro_name));
    assert_eq!(help.active_parameter, 1);
}

#[test]
fn resolve_fills_documentation_lazily() {
    let ctx = ctx();
    let mut docs = Documents::default();
    let doc = open_inline(&mut docs, &ctx, "file:///c.sql", "select * from {{ ref('");
    let items = features::completion(
        &ctx,
        doc,
        Position {
            line: 0,
            character: 22,
        },
    );
    let item = items
        .iter()
        .find(|i| i.label == "customers")
        .expect("customers item");
    assert_eq!(item.documentation, "", "docs are lazy now");
    let data = item.data.as_ref().expect("data payload");
    let resolved = features::resolve_completion_documentation(&ctx, data);
    assert!(resolved.is_some());
}

#[test]
fn workspace_symbols_filter() {
    let ctx = ctx();
    let all = features::workspace_symbols(&ctx, "");
    assert!(all.len() > 3);
    let filtered = features::workspace_symbols(&ctx, "customers");
    assert!(!filtered.is_empty());
    assert!(filtered
        .iter()
        .all(|s| s.name.to_lowercase().contains("customers")));
}

#[test]
fn yaml_diagnostics_syntax_and_clean() {
    let bad = "models:\n  - name: a\n   description: broken indent\n";
    let diags = yaml_features::yaml_diagnostics("file:///m/schema.yml", bad);
    assert!(!diags.is_empty(), "syntax error reported");

    let good = "version: 2\nmodels:\n  - name: a\n    description: fine\n";
    assert!(yaml_features::yaml_diagnostics("file:///m/schema.yml", good).is_empty());
}

#[test]
fn yaml_completion_offers_schema_keys() {
    let text = "version: 2\nmodels:\n  - name: a\n    ";
    let items = yaml_features::yaml_completion(
        "file:///m/schema.yml",
        text,
        Position {
            line: 3,
            character: 4,
        },
    );
    assert!(
        items.iter().any(|i| i.label == "description"),
        "model item keys offered"
    );
    let root = yaml_features::yaml_completion("file:///m/schema.yml", "", Position::default());
    assert!(root.iter().any(|i| i.label == "sources"));
}
