use super::*;

/// `(text, capture, layer)` of every span, the text read back from the
/// UTF-16 range.
fn spans_of(
    text: &str,
    language: HighlightLanguage,
    ctx: &YamlContext,
) -> Vec<(String, String, Layer)> {
    let h = highlight_with(text, language, 100_000, ctx).expect("grammars load");
    let lines: Vec<&str> = text.split('\n').collect();
    h.spans
        .iter()
        .map(|s| {
            assert_eq!(s.start.line, s.end.line, "{s:?} spans lines");
            let line: Vec<u16> = lines[s.start.line as usize].encode_utf16().collect();
            let piece = String::from_utf16_lossy(
                &line[s.start.character as usize..s.end.character as usize],
            );
            (piece, s.capture.clone(), s.layer)
        })
        .collect()
}

fn captures(text: &str, language: HighlightLanguage) -> Vec<(String, String)> {
    spans_of(text, language, &YamlContext::default())
        .into_iter()
        .map(|(p, c, _)| (p, c))
        .collect()
}

fn capture_of(found: &[(String, String)], piece: &str) -> String {
    found
        .iter()
        .find(|(p, _)| p == piece)
        .unwrap_or_else(|| panic!("no span for {piece:?} in {found:?}"))
        .1
        .clone()
}

/// A readable listing for snapshots: `text  capture  (layer)`.
fn listing(text: &str, ctx: &YamlContext) -> String {
    spans_of(text, HighlightLanguage::Yaml, ctx)
        .into_iter()
        .map(|(p, c, l)| format!("{p:<40} {c} ({})", l.name()))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn jinja_and_the_sql_between_the_tags_are_both_highlighted() {
    let found = captures(
        "select o.id from {{ ref('orders') }} o\n{% if is_incremental() %}\nwhere o.ts > 1\n{% endif %}",
        HighlightLanguage::Dbt,
    );
    assert_eq!(capture_of(&found, "{{"), "tag.delimiter");
    // dbt's own functions are builtins to the grammar
    assert_eq!(capture_of(&found, "ref"), "function.builtin");
    assert_eq!(capture_of(&found, "'orders'"), "string");
    assert_eq!(capture_of(&found, "if"), "keyword.conditional");
    // the SQL is the injected grammar's
    assert!(capture_of(&found, "select").starts_with("keyword"));
    assert!(capture_of(&found, "where").starts_with("keyword"));
}

#[test]
fn each_span_says_which_grammar_produced_it() {
    let found = spans_of(
        "select 1 from {{ ref('a') }}",
        HighlightLanguage::Dbt,
        &YamlContext::default(),
    );
    let layer = |piece: &str| found.iter().find(|(p, _, _)| p == piece).unwrap().2;
    assert_eq!(layer("select"), Layer::Sql);
    assert_eq!(layer("{{"), Layer::Jinja);
    let sql = spans_of("select 1", HighlightLanguage::Sql, &YamlContext::default());
    assert!(sql.iter().all(|(_, _, l)| *l == Layer::Sql), "{sql:?}");
}

#[test]
fn plain_sql_takes_the_sql_grammar_alone() {
    let found = captures(
        "select count(*) from analytics.orders",
        HighlightLanguage::Sql,
    );
    assert!(capture_of(&found, "select").starts_with("keyword"));
    assert!(!found.iter().any(|(_, c)| c == "tag.delimiter"));
}

#[test]
fn columns_count_utf16_code_units() {
    let h = highlight("-- 🦀\nselect 1", HighlightLanguage::Sql, 100).unwrap();
    let select = h
        .spans
        .iter()
        .find(|s| s.start.line == 1 && s.capture.starts_with("keyword"))
        .unwrap();
    assert_eq!((select.start.character, select.end.character), (0, 6));
    let comment = h
        .spans
        .iter()
        .find(|s| s.capture == "comment")
        .expect("the comment");
    // "-- 🦀" is 5 UTF-16 code units: the crab is a surrogate pair
    assert_eq!(comment.end.character, 5);
}

#[test]
fn spans_stop_at_the_cap() {
    let h = highlight("select a, b, c, d from t", HighlightLanguage::Sql, 2).unwrap();
    assert_eq!(h.spans.len(), 2);
    assert!(h.truncated);
}

#[test]
fn a_multi_line_span_is_split_at_line_breaks() {
    let h = highlight("/* one\ntwo */ select 1", HighlightLanguage::Sql, 100).unwrap();
    let comments: Vec<_> = h.spans.iter().filter(|s| s.capture == "comment").collect();
    assert_eq!(comments.len(), 2, "{comments:?}");
    assert!(h.spans.iter().all(|s| s.start.line == s.end.line));
}

#[test]
fn the_capture_names_are_known_and_clean() {
    let names = capture_names();
    for expected in [
        "tag.delimiter",
        "property",
        "type.dbt.model",
        "type.dbt.reference.unresolved",
    ] {
        assert!(
            names.iter().any(|n| n == expected),
            "{expected} in {names:?}"
        );
    }
    assert!(!names
        .iter()
        .any(|n| n.starts_with('_') || n == "spell" || n.starts_with('~')));
}

#[test]
fn language_names_round_trip_and_follow_the_path() {
    for l in [
        HighlightLanguage::Dbt,
        HighlightLanguage::Sql,
        HighlightLanguage::Yaml,
    ] {
        assert_eq!(HighlightLanguage::parse(l.name()), Some(l));
    }
    assert_eq!(HighlightLanguage::parse("python"), None);
    for (path, language) in [
        ("/p/models/a.sql", HighlightLanguage::Dbt),
        ("/p/models/schema.yml", HighlightLanguage::Yaml),
        ("/p/dbt_project.yaml", HighlightLanguage::Yaml),
        ("/p/target/compiled/p/models/a.sql", HighlightLanguage::Sql),
        ("target/run/p/models/a.sql", HighlightLanguage::Sql),
        ("C:\\p\\target\\compiled\\a.sql", HighlightLanguage::Sql),
        ("/p/macros/m.sql", HighlightLanguage::Dbt),
    ] {
        assert_eq!(HighlightLanguage::of_path(path), language, "{path}");
    }
}

#[test]
fn lines_filter_keeps_the_spans_that_touch_them() {
    let h = highlight("select 1\nfrom a\nwhere b", HighlightLanguage::Sql, 100).unwrap();
    let middle = h.in_lines(1, 2);
    assert!(
        !middle.is_empty() && middle.iter().all(|s| s.start.line == 1),
        "{middle:?}"
    );
}

#[test]
fn the_merge_paints_later_layers_over_earlier_ones() {
    let s = |start, end, capture: &str, layer| ByteSpan {
        start,
        end,
        capture: capture.into(),
        layer,
    };
    let merged = merge(
        vec![
            s(0, 10, "string", Layer::Yaml),
            s(2, 4, "keyword", Layer::Sql),
            s(3, 6, "tag.delimiter", Layer::Jinja),
            s(8, 10, "type.dbt.model", Layer::Dbt),
            s(8, 9, "keyword", Layer::Sql),
        ],
        12,
    );
    let got: Vec<_> = merged
        .iter()
        .map(|b| (b.start, b.end, b.capture.as_str()))
        .collect();
    assert_eq!(
        got,
        vec![
            (0, 2, "string"),
            (2, 3, "keyword"),
            (3, 6, "tag.delimiter"),
            (6, 8, "string"),
            (8, 10, "type.dbt.model"),
        ]
    );
}

const SCHEMA: &str = r#"version: 2

models:
  - name: customers   # the customers
    description: "{{ doc('customers') }} and more"
    config:
      materialized: table
      tags: ['nightly']
    columns:
      - name: customer_id
        data_tests:
          - unique
          - relationships:
              to: ref('stg_customers')
              field: customer_id
          - dbt_utils.expression_is_true:
              expression: "amount > 0"
              config:
                where: "{{ var('since') }} < created_at"
      - name: first_order
        description: |
          The first order, see {{ doc('orders') }}.

sources:
  - name: raw
    tables:
      - name: orders

unit_tests:
  - name: test_customers
    model: customers
    given:
      - input: ref('stg_customers')
        format: sql
        rows: |
          select 1 as customer_id
"#;

#[test]
fn a_property_file_is_coloured_by_every_layer() {
    let ctx = YamlContext {
        file: YamlFile::Properties,
        config_key: &|k: &str| matches!(k, "materialized" | "tags"),
        resolves: &|r: &Reference| !matches!(r, Reference::Doc(d) if d == "orders"),
    };
    insta::assert_snapshot!(listing(SCHEMA, &ctx));
}

#[test]
fn dbt_captures_mark_what_the_yaml_means() {
    let ctx = YamlContext::default();
    let found: Vec<(String, String)> = spans_of(SCHEMA, HighlightLanguage::Yaml, &ctx)
        .into_iter()
        .filter(|(_, _, l)| *l == Layer::Dbt)
        .map(|(p, c, _)| (p, c))
        .collect();
    for (piece, capture) in [
        ("customers", "type.dbt.model"),
        ("customer_id", "property.dbt.column"),
        ("unique", "function.dbt.test"),
        ("relationships", "function.dbt.test"),
        ("dbt_utils.expression_is_true", "function.dbt.test"),
        ("to", "variable.parameter.dbt"),
        ("field", "variable.parameter.dbt"),
        ("ref('stg_customers')", "type.dbt.reference"),
        ("customers", "type.dbt.model"),
        ("raw", "module.dbt.source"),
        ("orders", "type.dbt.source_table"),
        ("materialized", "keyword.dbt.config"),
        ("where", "keyword.dbt.config"),
    ] {
        assert!(
            found.iter().any(|(p, c)| p == piece && c == capture),
            "{piece} as {capture} in {found:?}"
        );
    }
    // the doc argument, not the call
    assert!(
        found
            .iter()
            .any(|(p, c)| p == "customers" && c == "string.special.dbt.doc"),
        "{found:?}"
    );
    // `config` and `arguments` of a test are no arguments
    assert!(!found
        .iter()
        .any(|(p, c)| p == "config" && c == "variable.parameter.dbt"));
}

#[test]
fn an_unresolved_reference_says_so() {
    let ctx = YamlContext {
        resolves: &|r: &Reference| !matches!(r, Reference::Model(m) if m == "nope"),
        ..YamlContext::default()
    };
    let found = spans_of("models:\n  - name: a\n    columns:\n      - name: id\n        data_tests:\n          - relationships:\n              to: ref('nope')\n", HighlightLanguage::Yaml, &ctx);
    assert!(
        found
            .iter()
            .any(|(p, c, _)| p == "ref('nope')" && c == "type.dbt.reference.unresolved"),
        "{found:?}"
    );
}

#[test]
fn jinja_in_values_is_coloured_and_prose_is_not_sql() {
    let found = spans_of(SCHEMA, HighlightLanguage::Yaml, &YamlContext::default());
    let at = |piece: &str| -> Vec<(String, Layer)> {
        found
            .iter()
            .filter(|(p, _, _)| p == piece)
            .map(|(_, c, l)| (c.clone(), *l))
            .collect()
    };
    assert!(
        at("{{")
            .iter()
            .all(|(c, l)| c == "tag.delimiter" && *l == Layer::Jinja),
        "{found:?}"
    );
    assert!(
        at("var").iter().any(|(_, l)| *l == Layer::Jinja),
        "{found:?}"
    );
    // "and more" after a doc() in a description stays a string, not SQL
    assert!(at("and").iter().all(|(_, l)| *l != Layer::Sql), "{found:?}");
}

#[test]
fn sql_keys_are_coloured_as_sql_with_expressions_unwrapped() {
    let found = spans_of(SCHEMA, HighlightLanguage::Yaml, &YamlContext::default());
    let sql: Vec<&str> = found
        .iter()
        .filter(|(_, _, l)| *l == Layer::Sql)
        .map(|(p, _, _)| p.as_str())
        .collect();
    // the expression `amount > 0` (wrapped in `select `, shifted back)
    assert!(sql.contains(&"amount"), "{sql:?}");
    assert!(!sql.contains(&"select "), "{sql:?}");
    // the where of a test's config, around its Jinja
    assert!(sql.contains(&"created_at"), "{sql:?}");
    // unit-test rows under format: sql, a block scalar
    assert!(sql.contains(&"select") && sql.contains(&"as"), "{sql:?}");
}

#[test]
fn an_unknown_key_with_sql_in_it_stays_a_string() {
    let found = spans_of(
        "models:\n  - name: a\n    description: select id from orders\n",
        HighlightLanguage::Yaml,
        &YamlContext::default(),
    );
    assert!(found.iter().all(|(_, _, l)| *l != Layer::Sql), "{found:?}");
}

#[test]
fn the_project_profiles_and_selectors_files_have_their_captures() {
    let project = YamlContext {
        file: YamlFile::Project,
        config_key: &|k: &str| k == "materialized",
        ..YamlContext::default()
    };
    let found = spans_of(
        "name: shop\nprofile: shop\nmodel-paths: [models]\nmodels:\n  shop:\n    +materialized: view\n    staging:\n      materialized: table\n",
        HighlightLanguage::Yaml,
        &project,
    );
    let dbt: Vec<(&str, &str)> = found
        .iter()
        .filter(|(_, _, l)| *l == Layer::Dbt)
        .map(|(p, c, _)| (p.as_str(), c.as_str()))
        .collect();
    for expected in [
        ("model-paths", "keyword.dbt.config"),
        ("+materialized", "keyword.dbt.config"),
        ("materialized", "keyword.dbt.config"),
        ("shop", "type.dbt.profile"),
    ] {
        assert!(dbt.contains(&expected), "{expected:?} in {dbt:?}");
    }

    let profiles = YamlContext {
        file: YamlFile::Profiles,
        ..YamlContext::default()
    };
    let found = spans_of(
        "shop:\n  target: dev\n  outputs:\n    dev:\n      type: duckdb\n",
        HighlightLanguage::Yaml,
        &profiles,
    );
    let dbt: Vec<&str> = found
        .iter()
        .filter(|(_, c, _)| c == "type.dbt.profile")
        .map(|(p, _, _)| p.as_str())
        .collect();
    assert_eq!(dbt, vec!["shop", "dev", "dev"], "{found:?}");

    let selectors = YamlContext {
        file: YamlFile::Selectors,
        ..YamlContext::default()
    };
    let found = spans_of(
        "selectors:\n  - name: nightly\n    definition: 'tag:nightly'\n",
        HighlightLanguage::Yaml,
        &selectors,
    );
    assert!(
        found
            .iter()
            .any(|(p, c, _)| p == "nightly" && c == "function.dbt.selector"),
        "{found:?}"
    );
}

#[test]
fn plain_yaml_keeps_the_base_layer() {
    let found = captures("a: 1\nb: true\nc: ~\n# note\n", HighlightLanguage::Yaml);
    assert_eq!(capture_of(&found, "a"), "property");
    assert_eq!(capture_of(&found, "1"), "number");
    assert_eq!(capture_of(&found, "true"), "boolean");
    assert_eq!(capture_of(&found, "# note"), "comment");
}
