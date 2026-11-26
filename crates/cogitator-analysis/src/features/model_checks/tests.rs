use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use cogitator_dialects::Dialect;
use cogitator_lsp::FixKind;
use pretty_assertions::assert_eq;
use rstest::rstest;

use super::*;
use crate::context::build_context;
use crate::Documents;

/// A dbt project on disk, removed when dropped.
struct Project {
    root: PathBuf,
}

impl Project {
    fn new(models: &[(&str, &str)], macros: &[(&str, &str)]) -> Self {
        static N: AtomicUsize = AtomicUsize::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let root =
            std::env::temp_dir().join(format!("cogitator_model_checks_{}_{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("models")).unwrap();
        std::fs::create_dir_all(root.join("macros")).unwrap();
        std::fs::write(
            root.join("dbt_project.yml"),
            "name: shop\nversion: '1.0'\nprofile: shop\nmodel-paths: ['models']\nmacro-paths: ['macros']\n",
        )
        .unwrap();
        for (name, sql) in models {
            std::fs::write(root.join("models").join(format!("{name}.sql")), sql).unwrap();
        }
        for (name, sql) in macros {
            std::fs::write(root.join("macros").join(format!("{name}.sql")), sql).unwrap();
        }
        Project { root }
    }

    fn uri(&self, model: &str) -> String {
        cogitator_lsp::fs_path_to_uri(&self.root.join("models").join(format!("{model}.sql")))
    }

    /// The diagnostics of `model` as the editor would hold it with `text`.
    fn check(&self, model: &str, text: &str) -> Vec<Diagnostic> {
        let ctx = build_context(self.root.to_str().unwrap());
        let uri = self.uri(model);
        let mut docs = Documents::default();
        docs.open(uri.clone(), text, &Dialect::default());
        model_diagnostics(&ctx, docs.get(&uri).unwrap(), &uri)
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// (code, severity, the text under the range, the suggestions)
fn summary(text: &str, diagnostics: &[Diagnostic]) -> Vec<(String, i32, String, Vec<String>)> {
    let offset = |p: &Position| {
        let line_start: usize = text
            .split_inclusive('\n')
            .take(p.line as usize)
            .map(str::len)
            .sum();
        line_start + p.character as usize
    };
    diagnostics
        .iter()
        .map(|d| {
            let code = d.code.clone();
            let span = text[offset(&d.range.start)..offset(&d.range.end)].to_string();
            let suggestions = d
                .data
                .as_ref()
                .map(|x| x.suggestions.clone())
                .unwrap_or_default();
            (code, d.severity, span, suggestions)
        })
        .collect()
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

#[rstest]
#[case::typo_of_a_builtin(
    "{{ config(materialized='incrementl') }}\nselect 1",
    vec![("invalid-materialization", ERROR, "incrementl", s(&["incremental"]))]
)]
#[case::unknown_without_near_miss(
    "{{ config(materialized='lakehouse_whatever') }}\nselect 1",
    vec![("invalid-materialization", WARNING, "lakehouse_whatever", vec![])]
)]
#[case::builtins_pass("{{ config(materialized='table') }}\nselect 1", vec![])]
#[case::adapter_ones_pass("{{ config(materialized='dynamic_table') }}\nselect 1", vec![])]
#[case::config_key_typo(
    "{{ config(materialised='table') }}\nselect 1",
    vec![("unknown-config-key", WARNING, "materialised", s(&["materialized"]))]
)]
#[case::adapter_keys_pass(
    "{{ config(materialized='table', partition_by={'field': 'd'}, my_own_key=1) }}\nselect 1",
    vec![]
)]
#[case::on_schema_change(
    "{{ config(materialized='incremental', on_schema_change='append_new_column') }}\nselect 1",
    vec![("invalid-config-value", WARNING, "append_new_column", s(&["append_new_columns"]))]
)]
#[case::access(
    "{{ config(access='publik') }}\nselect 1",
    vec![("invalid-config-value", ERROR, "publik", s(&["public"]))]
)]
#[case::strategy_typo(
    "{{ config(materialized='incremental', incremental_strategy='merg') }}\nselect 1",
    vec![("invalid-config-value", WARNING, "merg", s(&["merge"]))]
)]
#[case::boolean_as_string(
    "{{ config(enabled='false') }}\nselect 1",
    vec![("invalid-config-value", WARNING, "'false'", vec![])]
)]
#[case::never_incremental(
    "{{ config(materialized='table') }}\nselect 1 from t\n{% if is_incremental() %}where 1{% endif %}",
    vec![("is-incremental-never", WARNING, "is_incremental", vec![])]
)]
#[case::incremental_is_fine(
    "{{ config(materialized='incremental') }}\nselect 1 from t\n{% if is_incremental() %}where 1{% endif %}",
    vec![]
)]
#[case::empty_model(
    "{{ config(materialized='table') }}\n-- nothing yet\n",
    vec![("empty-model", WARNING, "{{ config(materialized='table') }}", vec![])]
)]
#[case::a_macro_call_is_sql("{{ build_it() }}", vec![])]
#[case::commented_out_config("{# {{ config(materialized='incrementl') }} #}\nselect 1", vec![])]
#[case::a_computed_value_is_left_alone(
    "{{ config(materialized=var('m', 'incrementl')) }}\nselect 1",
    vec![]
)]
fn config_findings(#[case] text: &str, #[case] expected: Vec<(&str, i32, &str, Vec<String>)>) {
    let project = Project::new(&[("orders", "select 1")], &[]);
    let found = summary(text, &project.check("orders", text));
    let expected: Vec<_> = expected
        .into_iter()
        .map(|(c, sev, span, sug)| (c.to_string(), sev, span.to_string(), sug))
        .collect();
    assert_eq!(found, expected);
}

#[test]
fn a_custom_materialization_is_known() {
    let project = Project::new(
        &[("orders", "select 1")],
        &[(
            "lake",
            "{% materialization lake_table, default %}{% endmaterialization %}",
        )],
    );
    let text = "{{ config(materialized='lake_table') }}\nselect 1";
    assert!(project.check("orders", text).is_empty());
    // and a typo of it is offered the custom one
    let text = "{{ config(materialized='lake_tabel') }}\nselect 1";
    let found = summary(text, &project.check("orders", text));
    assert_eq!(found[0].3, s(&["lake_table"]));
}

#[test]
fn a_custom_strategy_is_known() {
    let project = Project::new(
        &[("orders", "select 1")],
        &[(
            "strategy",
            "{% macro get_incremental_merj_sql(arg_dict) %}{% endmacro %}",
        )],
    );
    let text = "{{ config(materialized='incremental', incremental_strategy='merj') }}\nselect 1";
    assert!(project.check("orders", text).is_empty());
}

#[test]
fn a_self_ref_is_rewritten_to_this() {
    let project = Project::new(&[("orders", "select 1")], &[]);
    let text = "select * from {{ ref('orders') }}";
    let found = project.check("orders", text);
    assert_eq!(
        summary(text, &found),
        vec![("ref-self".to_string(), ERROR, "orders".to_string(), vec![])]
    );
    let Some(FixKind::MultiEdit { fixes }) = found[0].data.as_ref().map(|d| d.fix.clone()) else {
        panic!("a self ref carries its rewrite");
    };
    let edit = &fixes[0].edits[0];
    assert_eq!(edit.new_text, "this");
    // the edit replaces `ref('orders')`, from `ref` to its `)`
    assert_eq!(
        (edit.range.start.character, edit.range.end.character),
        (17, 30)
    );
}

#[test]
fn another_packages_model_of_the_same_name_is_no_self_ref() {
    let project = Project::new(&[("orders", "select 1")], &[]);
    let text = "select * from {{ ref('other_package', 'orders') }}";
    assert!(project.check("orders", text).is_empty());
}

#[test]
fn a_ref_that_closes_a_cycle_names_it() {
    let project = Project::new(
        &[
            ("orders", "select 1"),
            ("payments", "select * from {{ ref('customers') }}"),
            ("customers", "select * from {{ ref('orders') }}"),
        ],
        &[],
    );
    let text = "select * from {{ ref('payments') }}";
    let found = project.check("orders", text);
    assert_eq!(
        summary(text, &found),
        vec![(
            "ref-cycle".to_string(),
            ERROR,
            "payments".to_string(),
            vec![]
        )]
    );
    assert!(
        found[0]
            .message
            .contains("orders → payments → customers → orders"),
        "{}",
        found[0].message
    );
}

#[test]
fn a_ref_without_a_cycle_is_fine() {
    let project = Project::new(&[("orders", "select 1"), ("payments", "select 1")], &[]);
    assert!(project
        .check("orders", "select * from {{ ref('payments') }}")
        .is_empty());
}

#[test]
fn a_new_model_is_checked_before_it_is_saved() {
    let project = Project::new(&[("orders", "select 1")], &[]);
    let text = "{{ config(materialized='incrementl') }}\nselect 1";
    let found = summary(text, &project.check("not_saved_yet", text));
    assert_eq!(found[0].0, "invalid-materialization");
}

#[test]
fn only_models_are_checked() {
    let project = Project::new(&[("orders", "select 1")], &[]);
    let ctx = build_context(project.root.to_str().unwrap());
    let uri = cogitator_lsp::fs_path_to_uri(&project.root.join("analyses").join("a.sql"));
    let mut docs = Documents::default();
    let text = "{{ config(materialized='incrementl') }}";
    docs.open(uri.clone(), text, &Dialect::default());
    assert!(model_diagnostics(&ctx, docs.get(&uri).unwrap(), &uri).is_empty());
}

#[test]
fn the_message_says_what_to_do() {
    let project = Project::new(&[("orders", "select 1")], &[]);
    let text = "{{ config(materialized='incrementl') }}\nselect 1";
    let found = project.check("orders", text);
    assert_eq!(
        found[0].message,
        "materialization 'incrementl' does not exist — did you mean 'incremental'? \
         dbt fails to build the model."
    );
}
