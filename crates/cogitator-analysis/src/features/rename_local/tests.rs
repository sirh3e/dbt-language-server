use std::path::{Path, PathBuf};

use cogitator_dialects::Dialect;
use cogitator_lsp::{DocumentChange, Position};

use super::*;
use crate::context::MacroDetails;

const URI: &str = "file:///p/models/m.sql";

fn ctx() -> DbtContext {
    let mut ctx = DbtContext {
        dialect: Dialect::new("duckdb"),
        ..DbtContext::default()
    };
    ctx.project.name = "jaffle".into();
    ctx.model_detail_map
        .insert("orders".into(), Default::default());
    ctx
}

fn doc(text: &str) -> crate::Documents {
    let mut docs = crate::Documents::default();
    docs.open(URI.into(), text, &Dialect::new("duckdb"));
    docs
}

/// The position of the `n`th byte of the first `needle`.
fn at(text: &str, needle: &str, into: usize) -> Position {
    byte_position(text, (text.find(needle).unwrap() + into) as u32)
}

fn no_files(_: &Path) -> Option<String> {
    None
}

/// The text after applying the rename's edits to this file.
fn applied(text: &str, edit: &WorkspaceEdit) -> String {
    let DocumentChange::Edit(change) = &edit.document_changes[0] else {
        panic!("an edit first");
    };
    let mut out = text.to_string();
    let mut edits = change.edits.clone();
    edits.sort_by_key(|e| std::cmp::Reverse((e.range.start.line, e.range.start.character)));
    for e in edits {
        let off = |p: Position| names::offset_of(&out, p).unwrap() as usize;
        let (s, t) = (off(e.range.start), off(e.range.end));
        out.replace_range(s..t, &e.new_text);
    }
    out
}

fn rename(text: &str, needle: &str, new: &str) -> Option<Result<String, String>> {
    let (ctx, docs) = (ctx(), doc(text));
    let pos = at(text, needle, 0);
    rename_local_at(&ctx, docs.get(URI), URI, pos, new, &no_files)
        .map(|r| r.map(|edit| applied(text, &edit)))
}

#[test]
fn renames_in_a_model() {
    for (text, needle, new, want) in [
        (
            "{% set cols = ['a'] %}select {% for c in cols %}{{ c }}{% endfor %}",
            "cols =",
            "names",
            Some(Ok(
                "{% set names = ['a'] %}select {% for c in names %}{{ c }}{% endfor %}",
            )),
        ),
        (
            "with src as (select 1 as x) select src.x from src",
            "src.",
            "base",
            Some(Ok("with base as (select 1 as x) select base.x from base")),
        ),
        (
            "select o.id from t o",
            "o.",
            "ord",
            Some(Ok("select ord.id from t ord")),
        ),
        // a var() call is the var rename's, not a local name
        ("select {{ var('x') }}", "x'", "y", None),
        // a ref is the model rename's
        ("select * from {{ ref('orders') }}", "orders", "y", None),
    ] {
        let got = rename(text, needle, new);
        let want = want.map(|r: Result<&str, &str>| r.map(str::to_string).map_err(str::to_string));
        assert_eq!(got, want, "{text}");
    }
}

#[test]
fn refusals_say_why() {
    for (text, needle, new, says) in [
        ("{% set x = 1 %}{{ x }}", "x =", "2x", "not a valid"),
        ("{% set x = 1 %}{{ x }}{{ y }}", "x =", "y", "already used"),
        (
            "with a as (select 1) select * from a",
            "a as",
            "orders",
            "name of a model",
        ),
        (
            "with a as (select 1), b as (select 2) select * from a, b",
            "a as",
            "b",
            "CTE named",
        ),
        (
            "with a as (select 1) select * from a",
            "a as",
            "select",
            "not a valid",
        ),
        ("select o.x, o.{{ c }} from t o", "o.x", "p", "line 1"),
    ] {
        let got = rename(text, needle, new).expect("a local").expect_err(text);
        assert!(got.contains(says), "{text}: {got}");
    }
}

#[test]
fn a_template_local_macro_renames_its_keyword_arguments() {
    let text = "{% macro m(a, b=1) %}{{ b }}{% endmacro %}{{ m(1, b=2) }}{{ m(2) }}";
    let got = rename(text, "b=1", "c").unwrap().unwrap();
    assert_eq!(
        got,
        "{% macro m(a, c=1) %}{{ c }}{% endmacro %}{{ m(1, c=2) }}{{ m(2) }}"
    );
    let refused = rename(
        "{% macro m(a, b) %}{% endmacro %}{{ m(b=1, c=2) }}",
        "b)",
        "c",
    );
    assert!(refused.unwrap().is_err());
}

#[test]
fn package_macros_and_generic_tests_refuse_their_parameters() {
    let path = PathBuf::from("/p/dbt_packages/pkg/macros/m.sql");
    let uri = cogitator_lsp::fs_path_to_uri(&path);
    let text = "{% macro m(a) %}{{ a }}{% endmacro %}";
    let mut ctx = ctx();
    let details = MacroDetails {
        name: "m".into(),
        project_name: "pkg".into(),
        uri: path,
        ..MacroDetails::default()
    };
    ctx.macro_detail_map
        .entry("pkg".into())
        .or_default()
        .insert("m".into(), details);
    let mut docs = crate::Documents::default();
    docs.open(uri.clone(), text, &ctx.dialect);
    let pos = at(text, "a)", 0);
    let got = prepare_rename_local(&ctx, docs.get(&uri), &uri, pos).unwrap();
    assert!(got.unwrap_err().contains("installed package"));

    let test = "{% test pos(model, column_name) %}{{ column_name }}{% endtest %}";
    let got = rename(test, "column_name)", "col").unwrap();
    assert!(got.unwrap_err().contains("generic test"));
}

#[test]
fn prepare_highlights_and_references() {
    let text = "{% set x = 1 %}{{ x }}{{ x }}";
    let (ctx, docs) = (ctx(), doc(text));
    let pos = at(text, "x }}", 0);
    let prepared = prepare_rename_local(&ctx, docs.get(URI), URI, pos)
        .unwrap()
        .unwrap();
    assert_eq!(prepared.placeholder, "x");
    let kinds: Vec<i32> = local_highlights(&ctx, docs.get(URI), URI, pos)
        .unwrap()
        .iter()
        .map(|h| h.kind)
        .collect();
    assert_eq!(kinds, vec![3, 2, 2]);
    let with = local_references(&ctx, docs.get(URI), URI, pos, true).unwrap();
    let without = local_references(&ctx, docs.get(URI), URI, pos, false).unwrap();
    assert_eq!((with.len(), without.len()), (3, 2));
    // nothing local under the cursor answers None
    let sql = at(text, "{{", 0);
    assert!(local_highlights(&ctx, docs.get(URI), URI, sql).is_none());
}

#[test]
fn definition_jumps_to_the_declaration_in_scope() {
    let text = "with c as (select id, amount as total from {{ ref('orders') }})\n\
                select o.total from c o\n\
                where exists (select 1 from c o where o.id = 1)\n\
                order by total";
    let (ctx, docs) = (ctx(), doc(text));
    let doc = docs.get(URI).unwrap();
    let def = |needle: &str, into: usize| {
        let l = crate::features::definition(&ctx, doc, URI, at(text, needle, into));
        (l.range.start, l.range.end)
    };
    let span = |needle: &str, len: usize| (at(text, needle, 0), at(text, needle, len));
    for (needle, into, want) in [
        // a table alias, the outer one
        ("o.total", 0, span("o\nwhere", 1)),
        // the subquery's own alias of the same name
        ("o.id", 0, span("o where o.id", 1)),
        // a CTE from its use
        ("c o\nwhere", 0, span("c as", 1)),
        // a column through the CTE, and in ORDER BY
        ("total from c", 0, span("total from {{", 5)),
        ("by total", 3, span("total from {{", 5)),
        // on the alias itself: the column features answer (here, nothing)
        ("total from {{", 0, span("total from {{", 0)),
    ] {
        assert_eq!(def(needle, into), want, "{needle}");
    }
    // ref() keeps its own definition: the model's file
    let on_ref = crate::features::definition(&ctx, doc, URI, at(text, "orders", 0));
    assert_ne!(on_ref.uri, URI);
    // the alias, `o.total` and ORDER BY's `total` (an input column of `c`)
    let refs =
        local_references(&ctx, docs.get(URI), URI, at(text, "total from {{", 0), true).unwrap();
    assert_eq!(refs.len(), 3, "{refs:?}");
    // a column alias is never renamed here: rename_column owns it
    assert!(rename(text, "total from {{", "t").is_none());
}
