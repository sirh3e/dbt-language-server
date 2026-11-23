use super::*;
use crate::features::dispatch::testkit::{ctx, doc};

const MACROS: &str = concat!(
    "{% macro full_name(first, last, sep=' ') %}{{ first }}{{ sep }}{{ last }}{% endmacro %}\n",
    "{% macro anything() %}{{ varargs }}{{ kwargs }}{% endmacro %}\n",
);

fn codes(model: &str) -> Vec<(String, String)> {
    let ctx = ctx(
        "duckdb",
        &[
            ("root", "m.sql", MACROS),
            ("pkg", "p.sql", "{% macro p_only(x) %}{% endmacro %}"),
        ],
    );
    check(&ctx, &doc(model))
        .into_iter()
        .map(|d| (d.code, d.message))
        .collect()
}

#[test]
fn call_checks_case_table() {
    let cases: &[(&str, &[&str])] = &[
        ("{{ full_name('a', 'b') }}", &[]),
        ("{{ full_name('a', 'b', '-') }}", &[]),
        ("{{ full_name('a', 'b', '-', 'x') }}", &[ARG_COUNT]),
        ("{{ full_name('a') }}", &[ARG_COUNT]),
        ("{{ full_name(first='a', last='b') }}", &[]),
        ("{{ full_name('a', lst='b') }}", &[UNKNOWN_KWARG, ARG_COUNT]),
        (
            "{{ full_name('a', 'b', sep='-', sep='+') }}",
            &[DUPLICATE_KWARG],
        ),
        // inside a block, qualified, and in a package
        ("{% set x = root.full_name('a') %}", &[ARG_COUNT]),
        ("{% if pkg.p_only() %}{% endif %}", &[ARG_COUNT]),
        // lenient bodies, splats, unparsable and unresolved calls
        ("{{ anything(1, 2, z=3) }}", &[]),
        ("{{ full_name(*xs) }}", &[]),
        ("{{ full_name('a',, ) }}", &[]),
        ("{{ nowhere(1, 2, 3) }}", &[]),
        // dbt builtins are never checked
        ("{{ run_query('select 1', 2, 3) }}", &[]),
    ];
    for (model, want) in cases {
        let got: Vec<String> = codes(model).into_iter().map(|(c, _)| c).collect();
        assert_eq!(got, *want, "{model}");
    }
}

#[test]
fn unknown_kwarg_offers_the_rename() {
    let ctx = ctx("duckdb", &[("root", "m.sql", MACROS)]);
    let model = "{{ full_name('a', lst='b') }}";
    let diagnostics = check(&ctx, &doc(model));
    let unknown = diagnostics
        .iter()
        .find(|d| d.code == UNKNOWN_KWARG)
        .unwrap();
    assert!(
        unknown.message.contains("did you mean 'last'"),
        "{}",
        unknown.message
    );
    assert_eq!(unknown.range.start.character, 18);
    assert_eq!(unknown.range.end.character, 21);
    let data = unknown.data.as_ref().unwrap();
    assert_eq!(data.suggestions, vec!["last".to_string()]);
}
