//! Macro calls anywhere in the Jinja layer: statement blocks, nested
//! arguments, filter arguments and `{% call %}` blocks are call tokens
//! like `{{ m() }}`; context functions, methods, filters, tests,
//! keywords and names the template binds itself are not.

use cogitator_dialects::Dialect;
use cogitator_parser::{DocumentParser, JinjaSqlParser, TokenKind};
use pretty_assertions::assert_eq;
use rstest::rstest;

/// `(package, macro)` of every macro call token, in order.
fn calls(text: &str) -> Vec<(Option<String>, String)> {
    let doc = JinjaSqlParser
        .parse(text, &Dialect::new("duckdb"))
        .expect("parse ok");
    doc.tokens
        .iter()
        .enumerate()
        .filter(|(_, t)| t.kind == TokenKind::Macro)
        .map(|(i, t)| {
            let package = i
                .checked_sub(2)
                .map(|p| &doc.tokens[p])
                .filter(|p| p.kind == TokenKind::Package)
                .map(|p| doc.literal(p).to_string());
            (package, doc.literal(t).to_string())
        })
        .collect()
}

fn plain(names: &[&str]) -> Vec<(Option<String>, String)> {
    names.iter().map(|n| (None, n.to_string())).collect()
}

#[rstest]
#[case::expression("{{ my_macro(1) }}", &["my_macro"])]
#[case::set("{% set x = my_macro(a) %}", &["my_macro"])]
#[case::if_block("{% if my_macro() %}x{% endif %}", &["my_macro"])]
#[case::elif_block("{% if a %}{% elif not my_macro() %}{% endif %}", &["my_macro"])]
#[case::for_block("{% for c in get_cols(ref('x')) %}{{ c }}{% endfor %}", &["get_cols"])]
#[case::do_block("{% do my_macro() %}", &["my_macro"])]
#[case::nested("{{ outer(inner()) }}", &["outer", "inner"])]
#[case::filter_argument("{{ x | default(my_macro()) }}", &["my_macro"])]
#[case::call_block("{% call my_macro() %}select 1{% endcall %}", &["my_macro"])]
#[case::call_with_caller_args("{% call(row) my_macro(1) %}{{ row }}{% endcall %}", &["my_macro"])]
#[case::keyword_argument("{{ outer(arg=inner()) }}", &["outer", "inner"])]
#[case::in_parens("{% if a and (my_macro()) %}{% endif %}", &["my_macro"])]
#[case::macro_body(
    "{% macro a(x) %}{% set y = b(x) %}{{ return(y) }}{% endmacro %}",
    &["b"]
)]
#[case::default_of_a_parameter("{% macro a(x=b()) %}{% endmacro %}", &["b"])]
#[case::is_incremental("{% if is_incremental() %}x{% endif %}", &["is_incremental"])]
fn unqualified_calls(#[case] text: &str, #[case] want: &[&str]) {
    assert_eq!(calls(text), plain(want), "{text}");
}

#[rstest]
#[case::set("{% set c = dbt_utils.star(ref('x')) %}", "dbt_utils", "star")]
#[case::if_block("{% if pkg.check() %}{% endif %}", "pkg", "check")]
#[case::for_block("{% for c in pkg.cols(ref('x')) %}{% endfor %}", "pkg", "cols")]
#[case::do_block("{% do pkg.run_it() %}", "pkg", "run_it")]
#[case::nested("{{ outer(pkg.inner()) }}", "pkg", "inner")]
#[case::call_block("{% call pkg.wrap() %}x{% endcall %}", "pkg", "wrap")]
fn qualified_calls(#[case] text: &str, #[case] package: &str, #[case] name: &str) {
    let found = calls(text);
    assert!(
        found.contains(&(Some(package.into()), name.into())),
        "{text}: {found:?}"
    );
}

#[rstest]
#[case::context_functions("{{ return(1) }}{% do log('x') %}{% for i in range(3) %}{% endfor %}")]
#[case::dbt_functions("{% set a = env_var('X') %}{% set b = var('y') %}{% set c = ref('m') %}")]
#[case::source_and_config("{% set s = source('a', 'b') %}{% set m = config.get('x') %}")]
#[case::caller("{% macro m() %}{{ caller() }}{% endmacro %}")]
#[case::context_objects(
    "{% do adapter.dispatch('x', 'p')() %}{% do exceptions.raise_compiler_error('e') %}"
)]
#[case::more_objects("{{ this.render() }}{% for x in y %}{{ loop.cycle('a', 'b') }}{% endfor %}")]
#[case::modules("{% set t = modules.datetime.datetime.now() %}")]
#[case::value_methods("{% for k, v in d.items() %}{% endfor %}{% set p = s.split(',') %}")]
#[case::set_local("{% set f = x %}{{ f(1) }}{% set r = x %}{{ r.columns.keys() }}{{ r.helper() }}")]
#[case::for_local("{% for f in fs %}{{ f() }}{% endfor %}")]
#[case::macro_parameter("{% macro m(fn, cols) %}{{ fn() }}{{ cols.thing() }}{% endmacro %}")]
#[case::imported("{% from 'x.sql' import helper %}{{ helper() }}")]
#[case::filters("{{ x | default(1) }}{{ x | replace('a', 'b') }}")]
#[case::tests("{% if x is divisibleby(3) %}{% endif %}{% if x is not sameas(y) %}{% endif %}")]
#[case::keywords(
    "{% if (a) %}{% endif %}{% for c in (a, b) %}{% endfor %}{% if not (a) %}{% endif %}"
)]
#[case::definition_names("{% macro m(a) %}{% endmacro %}{% test t(model) %}{% endtest %}")]
#[case::comments("{# my_macro() #}select 1")]
#[case::sql("select coalesce(a, b), count(*) from t")]
#[case::plain_variable("{{ some_var }}{% set x = y %}")]
fn not_macro_calls(#[case] text: &str) {
    let found = calls(text);
    assert!(found.is_empty(), "{text}: {found:?}");
}
