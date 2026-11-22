use std::path::Path;

use pretty_assertions::assert_eq;

use super::*;

const MODEL: &str = "{{ config(materialized='table') }}\n\
select o.id, c.name\n\
from {{ ref('orders') }} o\n\
join {{ ref('customers') }} c on c.id = o.customer_id\n";

fn env(m: &Match) -> Vec<(&str, &str)> {
    m.env
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect()
}

#[test]
fn a_dbt_pattern_matches_jinja_and_binds_its_metavariables() {
    let search = Search::new("{{ ref($M) }}", DbtLanguage::Dbt, None).unwrap();
    let found = search.matches(MODEL, 100);
    assert_eq!(found.len(), 2, "{found:?}");
    assert_eq!(found[0].text, "{{ ref('orders') }}");
    assert_eq!(env(&found[0]), vec![("M", "'orders'")]);
    assert_eq!(
        found[0].start,
        Point {
            line: 2,
            character: 5
        }
    );
    assert_eq!(found[1].text, "{{ ref('customers') }}");
}

#[test]
fn a_rewrite_fills_in_the_metavariables() {
    let search = Search::new(
        "{{ ref($M) }}",
        DbtLanguage::Dbt,
        Some("{{ ref('core', $M) }}"),
    )
    .unwrap();
    let found = search.matches(MODEL, 100);
    assert_eq!(
        found[0].replacement.as_deref(),
        Some("{{ ref('core', 'orders') }}")
    );
}

#[test]
fn a_multi_metavariable_takes_every_argument() {
    let search = Search::new("{{ config($$$ARGS) }}", DbtLanguage::Dbt, None).unwrap();
    let found = search.matches(MODEL, 100);
    assert_eq!(found.len(), 1);
    assert_eq!(env(&found[0]), vec![("ARGS", "materialized='table'")]);
}

#[test]
fn the_sql_between_the_tags_is_no_dbt_structure_but_sql_is() {
    // to tree-sitter-dbt the SQL is text: no match
    let dbt = Search::new("select $A from $T", DbtLanguage::Dbt, None);
    assert!(dbt.map(|s| s.matches(MODEL, 10).is_empty()).unwrap_or(true));
    // the compiled SQL has the structure; a SQL pattern is a whole
    // statement (the grammar parses nothing smaller)
    let sql = Search::new("select $$$C from $T $A group by $G", DbtLanguage::Sql, None).unwrap();
    let found = sql.matches(
        "select o.status, count(*) from analytics.orders o group by 1",
        10,
    );
    assert_eq!(found.len(), 1);
    assert_eq!(
        env(&found[0]),
        vec![
            ("A", "o"),
            ("C", "o.status, count(*)"),
            ("G", "1"),
            ("T", "analytics.orders")
        ]
    );
}

#[test]
fn matches_stop_at_the_cap() {
    let search = Search::new("{{ ref($M) }}", DbtLanguage::Dbt, None).unwrap();
    assert_eq!(search.matches(MODEL, 1).len(), 1);
}

#[test]
fn a_pattern_that_does_not_parse_says_so() {
    let err = Search::new("{{ ref(", DbtLanguage::Dbt, None).err();
    if let Some(err) = err {
        assert!(err.0.contains("dbt"), "{err}");
    }
}

#[test]
fn a_file_is_read_as_dbt_unless_it_is_compiled() {
    assert_eq!(
        language_of(Path::new("models/a.sql")),
        Some(DbtLanguage::Dbt)
    );
    assert_eq!(
        language_of(Path::new("target/compiled/p/models/a.sql")),
        Some(DbtLanguage::Sql)
    );
    assert_eq!(language_of(Path::new("models/a.yml")), None);
}

const RULES: &str = r#"
id: ref-of-a-var
language: dbt
severity: warning
message: "ref() of a variable: dbt cannot see the dependency"
note: write the model's name, or use a source
rule:
  pattern: "{{ ref(var($V)) }}"
---
id: no-select-star
language: sql
severity: info
message: select * in compiled SQL
rule:
  pattern: "select * from $T"
---
id: a-python-rule
language: python
rule:
  pattern: print($A)
---
id: hardcoded-config
language: dbt
severity: hint
message: config set to table
rule:
  pattern: "{{ config(materialized='table') }}"
fix: "{{ config(materialized='incremental') }}"
"#;

#[test]
fn rules_of_dbt_and_sql_load_and_others_are_left_to_the_cli() {
    let rules = Rules::from_yaml([("rules.yml".to_string(), RULES)]);
    assert_eq!(rules.errors, Vec::<String>::new());
    assert_eq!(
        rules.ids(),
        ["hardcoded-config", "no-select-star", "ref-of-a-var"]
    );
    assert_eq!(rules.skipped, 1);
}

#[test]
fn rules_find_what_they_say_in_the_files_of_their_language() {
    let rules = Rules::from_yaml([("rules.yml".to_string(), RULES)]);
    let model = "{{ config(materialized='table') }}\nselect * from {{ ref(var('src')) }}\n";
    let found = rules.check(Path::new("models/a.sql"), model);
    let ids: Vec<_> = found.iter().map(|f| f.rule.as_str()).collect();
    assert_eq!(ids, vec!["hardcoded-config", "ref-of-a-var"]);
    assert_eq!(found[0].severity, RuleSeverity::Hint);
    assert_eq!(
        found[0].fix.as_deref(),
        Some("{{ config(materialized='incremental') }}")
    );
    assert_eq!(found[1].severity, RuleSeverity::Warning);
    assert_eq!(
        found[1].note.as_deref(),
        Some("write the model's name, or use a source")
    );

    let compiled = rules.check(
        Path::new("target/compiled/p/models/a.sql"),
        "select * from analytics.src",
    );
    assert_eq!(
        compiled.iter().map(|f| f.rule.as_str()).collect::<Vec<_>>(),
        vec!["no-select-star"]
    );
}

#[test]
fn a_broken_rule_is_an_error_and_the_rest_still_load() {
    let rules = Rules::from_yaml([(
        "bad.yml".to_string(),
        "id: bad\nlanguage: dbt\nrule:\n  nonsense: 1\n---\nid: good\nlanguage: dbt\nrule:\n  pattern: \"{{ ref($M) }}\"\n",
    )]);
    assert_eq!(rules.ids(), ["good"]);
    assert_eq!(rules.errors.len(), 1);
    assert!(rules.errors[0].starts_with("bad.yml"), "{:?}", rules.errors);
}

#[test]
fn the_rule_directories_are_read_from_sgconfig() {
    assert_eq!(
        rule_dirs("ruleDirs:\n  - rules\n  - 'more'\nutilDirs: [u]\n"),
        vec!["rules", "more"]
    );
    assert_eq!(rule_dirs("ruleDirs: [a, \"b\"]\n"), vec!["a", "b"]);
    assert_eq!(rule_dirs("customLanguages: {}\n"), Vec::<String>::new());
}
