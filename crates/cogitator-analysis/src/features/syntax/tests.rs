use cogitator_dialects::Dialect;
use cogitator_lsp::Diagnostic;

use super::*;

const URI: &str = "file:///p/models/m.sql";

fn check(text: &str) -> Vec<Diagnostic> {
    syntax_diagnostics(text, URI, &Dialect::new("duckdb"), SyntaxOptions::default())
}

/// (code, line, character) of the single diagnostic, if any.
fn found(text: &str) -> Option<(String, u32, u32)> {
    let items = check(text);
    assert!(items.len() <= 1, "{items:?}");
    items
        .first()
        .map(|d| (d.code.clone(), d.range.start.line, d.range.start.character))
}

#[test]
fn jinja_errors_are_found_where_they_are() {
    for (text, line, character) in [
        // unclosed blocks point at their opener
        ("select 1\n{% if x %}\nselect 2\n", 1, 0),
        ("{% for r in rows %}\n{{ r }}", 0, 0),
        ("{% macro m() %}\nselect 1\n", 0, 0),
        ("{% if a %}{% for b in c %}{% endfor %}", 0, 0),
        // a closing tag no block opened
        ("select 1\n{% endif %}\n", 1, 0),
        ("{% for x in y %}{% endfor %}{% endfor %}", 0, 28),
        // unbalanced delimiters
        ("select {{ ref('a') \nfrom t", 1, 0),
        ("select 1 {# never closed", 0, 9),
        ("select {{ 'abc }}", 0, 10),
        // malformed expressions
        ("select {{ ref('a' }}", 0, 18),
        ("select {{ x + }}", 0, 14),
        ("{% if %}x{% endif %}", 0, 6),
    ] {
        let got = found(text).unwrap_or_else(|| panic!("no error for {text:?}"));
        assert_eq!(got.0, JINJA_CODE, "{text:?}");
        assert_eq!((got.1, got.2), (line, character), "{text:?}");
    }
}

#[test]
fn valid_jinja_and_dbt_tags_give_nothing() {
    for text in [
        "select {{ ref('a') }} from {{ source('s', 't') }}",
        "{{ config(materialized='table') }}\nselect 1",
        "{% set x = 1 %}select {{ x }}",
        "{% set q %}select 1{% endset %}{{ q }}",
        "{%- if is_incremental() -%} where a > 1 {%- endif %}",
        "{% test not_null(model, column_name) %}select 1{% endtest %}",
        "{% snapshot s %}select 1{% endsnapshot %}",
        "{% materialization m, default %}{{ return([]) }}{% endmaterialization %}",
        "{% docs d %}some *text*{% enddocs %}",
        "{% macro m(a, b=2) %}{% do log(a) %}{{ caller() }}{% endmacro %}",
        "{% call statement('x') %}select 1{% endcall %}",
        "select '{{ var(\"x\") }}' as v -- }} in sql",
        "select 1 {# comment {{ #}",
        "select {% raw %}'{{ not jinja }}'{% endraw %}",
        // dbt-only or extension statements: declined, not errors
        "{% for x in y %}{% break %}{% endfor %}",
        "{% for x in y %}{% if x %}{% continue %}{% endif %}{% endfor %}",
    ] {
        assert_eq!(found(text), None, "{text:?}");
    }
}

#[test]
fn sql_errors_are_found_where_they_are() {
    for (text, line, character) in [
        ("select a,, b from t", 0, 9),
        ("select a from t where", 0, 21),
        ("select a\nfrom {{ ref('t') }}\nwhere a = = 1", 2, 10),
        ("{{ config(materialized='view') }}\nselect (1 from t", 1, 10),
        ("select a from t group by order by a", 0, 31),
        ("{# note #}\nselec 1", 1, 0),
    ] {
        let got = found(text).unwrap_or_else(|| panic!("no error for {text:?}"));
        assert_eq!(got.0, SQL_CODE, "{text:?}");
        assert_eq!((got.1, got.2), (line, character), "{text:?}");
    }
}

#[test]
fn uncertain_sql_declines() {
    for text in [
        // control flow changes the SQL's shape
        "select a {% if x %}, b{% endif %} from t",
        "{% for c in cols %}{{ c }},{% endfor %} from t",
        "select 1 {% if is_incremental() %} where {% endif %}",
        // a block set's body is not SQL
        "{% set q %} not sql at all ( {% endset %}select 1",
        // errors next to masked expressions: they render to anything
        "{{ dbt_utils.union_relations(relations=[ref('a'), ref('b')]) }}",
        "select a {{ op }} b from t",
        "select * from t {{ where_clause() }} order by 1",
        "{{ select_all() }} where 1 = 1",
        // valid in some warehouse sqlparser knows
        "select * exclude (a) from t",
        "select a from t qualify row_number() over (partition by a) = 1",
        "select `a` from t",
        // only Jinja, no SQL
        "{{ config(materialized='ephemeral') }}",
        "",
    ] {
        assert_eq!(found(text), None, "{text:?}");
    }
}

#[test]
fn messages_carry_a_reason_and_a_solution() {
    for text in ["select 1\n{% if x %}", "select a,, b from t"] {
        let items = check(text);
        let message = &items[0].message;
        assert!(message.contains("\nReason: "), "{message}");
        assert!(message.contains("\nSolution: "), "{message}");
        assert_eq!(items[0].source, "cogitator");
        assert_eq!(items[0].severity, cogitator_lsp::diagnostic_severity::ERROR);
    }
}

#[test]
fn only_dbt_sql_files_are_checked() {
    let broken = "select a,, b";
    for (uri, checked) in [
        ("file:///p/models/m.sql", true),
        ("file:///p/macros/m.sql", true),
        ("file:///p/models/schema.yml", false),
        ("file:///p/docs/d.md", false),
        ("file:///p/target/compiled/p/models/m.sql", false),
        ("file:///p/target/run/p/models/m.sql", false),
    ] {
        let items = syntax_diagnostics(
            broken,
            uri,
            &Dialect::new("duckdb"),
            SyntaxOptions::default(),
        );
        assert_eq!(!items.is_empty(), checked, "{uri}");
    }
}

#[test]
fn the_sql_layer_can_be_left_to_another_engine() {
    let options = SyntaxOptions { sql: false };
    let dialect = Dialect::new("duckdb");
    assert!(syntax_diagnostics("select a,, b", URI, &dialect, options).is_empty());
    assert_eq!(
        syntax_diagnostics("select {{ a", URI, &dialect, options).len(),
        1,
        "Jinja is still checked"
    );
}

#[test]
fn every_fixture_sql_file_is_clean() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
    let mut checked = 0;
    for entry in walkdir::WalkDir::new(&root) {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("sql") {
            continue;
        }
        let text = std::fs::read_to_string(path).unwrap();
        let uri = cogitator_lsp::fs_path_to_uri(path);
        for dialect in ["duckdb", "postgres", "snowflake", ""] {
            let items = syntax_diagnostics(
                &text,
                &uri,
                &Dialect::new(dialect),
                SyntaxOptions::default(),
            );
            assert!(
                items.is_empty(),
                "{} ({dialect}): {items:?}",
                path.display()
            );
        }
        checked += 1;
    }
    assert!(checked >= 90, "only {checked} fixture files found");
}
