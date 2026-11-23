use cogitator_lsp::Position;
use cogitator_parser::macro_body::DispatchCall;

use super::implementation::{dispatch_at_cursor, implementation, with_dispatch_line};
use super::testkit::{ctx, doc};
use super::*;

const ROOT: &str = concat!(
    "{% macro my_x(a) %}{{ return(adapter.dispatch('my_x', 'root')(a)) }}{% endmacro %}\n",
    "{% macro default__my_x(a) %}d{% endmacro %}\n",
    "{% macro duckdb__my_x(a) %}k{% endmacro %}\n",
);
const PKG: &str =
    "{% macro postgres__my_x(a) %}p{% endmacro %}{% macro default__my_x(a) %}pd{% endmacro %}";

fn call(name: &str, ns: Option<&str>) -> DispatchCall {
    DispatchCall {
        name: name.into(),
        namespace: ns.map(Into::into),
    }
}

#[test]
fn search_packages_case_table() {
    let mut c = ctx("duckdb", &[("root", "m.sql", ROOT), ("pkg", "p.sql", PKG)]);
    assert_eq!(search_packages(&c, Some("pkg")), ["root", "pkg", "dbt"]);
    assert_eq!(search_packages(&c, None), ["root", "pkg", "dbt"]);
    c.project.dispatch = vec![cogitator_schemas::DispatchConfig {
        macro_namespace: "pkg".into(),
        search_order: vec!["pkg".into(), "root".into()],
    }];
    assert_eq!(search_packages(&c, Some("pkg")), ["pkg", "root", "dbt"]);
}

#[test]
fn adapter_prefixes_case_table() {
    let cases: &[(&str, &[&str])] = &[
        ("duckdb", &["duckdb", "default"]),
        ("redshift", &["redshift", "postgres", "default"]),
        ("databricks", &["databricks", "spark", "default"]),
        ("", &["default"]),
    ];
    for (adapter, want) in cases {
        assert_eq!(adapter_prefixes(adapter), *want, "{adapter}");
    }
}

/// (adapter, namespace, configured order, picked package.macro)
type PickCase = (
    &'static str,
    Option<&'static str>,
    Option<&'static [&'static str]>,
    Option<&'static str>,
);

#[test]
fn active_implementation_case_table() {
    let cases: &[PickCase] = &[
        ("duckdb", Some("root"), None, Some("root.duckdb__my_x")),
        ("snowflake", Some("root"), None, Some("root.default__my_x")),
        // package-major: the root's default beats the package's postgres
        ("postgres", Some("pkg"), None, Some("root.default__my_x")),
        (
            "postgres",
            Some("pkg"),
            Some(&["pkg", "root"]),
            Some("pkg.postgres__my_x"),
        ),
        // redshift falls back to its postgres parent
        (
            "redshift",
            Some("pkg"),
            Some(&["pkg"]),
            Some("pkg.postgres__my_x"),
        ),
        ("duckdb", Some("dbt"), Some(&["dbt"]), None),
    ];
    for (adapter, ns, order, want) in cases {
        let mut c = ctx(adapter, &[("root", "m.sql", ROOT), ("pkg", "p.sql", PKG)]);
        if let (Some(ns), Some(order)) = (ns, order) {
            c.project.dispatch = vec![cogitator_schemas::DispatchConfig {
                macro_namespace: ns.to_string(),
                search_order: order.iter().map(|s| s.to_string()).collect(),
            }];
        }
        let got = active_implementation(&c, &call("my_x", *ns))
            .map(|i| format!("{}.{}", i.package, i.details.name));
        assert_eq!(got.as_deref(), *want, "{adapter} {ns:?} {order:?}");
    }
}

#[test]
fn implementations_lists_every_prefix_but_not_doubled_names() {
    let c = ctx(
        "duckdb",
        &[
            ("root", "m.sql", ROOT),
            ("pkg", "p.sql", PKG),
            ("o", "o.sql", "{% macro a__b__my_x() %}{% endmacro %}"),
        ],
    );
    let names: Vec<String> = implementations(&c, "my_x")
        .iter()
        .map(|i| format!("{}.{}", i.package, i.details.name))
        .collect();
    assert_eq!(
        names,
        [
            "pkg.default__my_x",
            "pkg.postgres__my_x",
            "root.default__my_x",
            "root.duckdb__my_x"
        ]
    );
}

#[test]
fn the_cursor_finds_its_dispatch_three_ways() {
    let c = ctx("duckdb", &[("root", "m.sql", ROOT), ("pkg", "p.sql", PKG)]);
    let at = |text: &str, character: u32| {
        dispatch_at_cursor(&c, &doc(text), Position { line: 0, character })
    };
    let want = Some(call("my_x", Some("root")));
    // on the dispatch call itself
    assert_eq!(at("{{ adapter.dispatch('my_x', 'root')() }}", 5), want);
    // on a call of the dispatching macro
    assert_eq!(at("{{ my_x(1) }}", 4), want);
    // on the definition header
    assert_eq!(at(ROOT, 10), want);
    // elsewhere: nothing
    assert_eq!(at("{{ my_x(1) }}", 0), None);
}

#[test]
fn implementation_orders_by_dispatch_and_hover_names_the_pick() {
    let c = ctx("duckdb", &[("root", "m.sql", ROOT), ("pkg", "p.sql", PKG)]);
    let d = doc("{{ my_x(1) }}");
    let pos = Position {
        line: 0,
        character: 4,
    };
    let uris: Vec<String> = implementation(&c, &d, pos)
        .into_iter()
        .map(|l| l.uri)
        .collect();
    assert_eq!(uris.len(), 4);
    assert!(uris[0].ends_with("/root/macros/m.sql"), "{uris:?}");
    assert!(uris[3].ends_with("/pkg/macros/p.sql"), "{uris:?}");
    let hover = with_dispatch_line(&c, &d, pos, "my_x(a)".into());
    assert_eq!(
        hover,
        "my_x(a)\n\nDispatches to `root.duckdb__my_x` for adapter `duckdb`."
    );
}

#[test]
fn no_default_case_table() {
    let cases: &[(&str, &str, usize)] = &[
        ("{{ adapter.dispatch('my_x', 'root')() }}", "snowflake", 0),
        ("{{ adapter.dispatch('nope', 'root')() }}", "duckdb", 1),
        // an unknown package or dbt's own namespace is not judged
        ("{{ adapter.dispatch('nope', 'other')() }}", "duckdb", 0),
        ("{{ adapter.dispatch('nope', 'dbt')() }}", "duckdb", 0),
        // no namespace: only names the project implements somewhere
        ("{{ adapter.dispatch('current_timestamp')() }}", "duckdb", 0),
        ("{{ adapter.dispatch('only_pg')() }}", "duckdb", 1),
    ];
    for (text, adapter, want) in cases {
        let c = ctx(
            adapter,
            &[
                ("root", "m.sql", ROOT),
                (
                    "pkg",
                    "p.sql",
                    "{% macro postgres__only_pg() %}{% endmacro %}",
                ),
            ],
        );
        let got = no_default::check(&c, &doc(text));
        assert_eq!(got.len(), *want, "{text} on {adapter}");
        for d in got {
            assert_eq!(d.code, no_default::CODE);
        }
    }
}
