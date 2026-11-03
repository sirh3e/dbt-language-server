//! textDocument/codeAction over the fixture: the shared model command set,
//! the schema-gated tail, the kind filter, ref-targeted actions, and the
//! YAML-entry actions.

use std::path::PathBuf;

use cogitator_analysis::{build_context, features, Documents};
use cogitator_lsp::{Position, Range};
use pretty_assertions::assert_eq;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/jaffle_shop_duckdb")
}

fn ctx() -> cogitator_analysis::DbtContext {
    build_context(fixture_root().to_str().unwrap())
}

fn model_uri(rel: &str) -> String {
    cogitator_lsp::fs_path_to_uri(&fixture_root().join(rel))
}

fn titles(actions: &[cogitator_lsp::CodeAction]) -> Vec<String> {
    actions.iter().map(|a| a.title.clone()).collect()
}

#[test]
fn model_file_offers_the_lens_command_set() {
    let ctx = ctx();
    let uri = model_uri("models/customers.sql");
    let actions = features::code_actions(&ctx, None, &uri, Range::default(), None);
    let titles = titles(&actions);
    assert!(titles.contains(&"Go to schema".to_string()), "{titles:?}");
    assert!(titles.contains(&"dbt run".to_string()), "{titles:?}");
    // customers has a schema entry with tests: the goToTests tail, not the
    // extract scaffold
    assert!(
        actions.iter().any(|a| a.command.command == "dbt.goToTests"),
        "{titles:?}"
    );
    assert!(
        !titles.contains(&"Extract schema YAML".to_string()),
        "{titles:?}"
    );
    for a in &actions {
        assert!(
            a.command.command.starts_with("dbt."),
            "{}",
            a.command.command
        );
        let arg = serde_json::to_value(&a.command.arguments[0]).unwrap();
        assert_eq!(arg["uri"], serde_json::Value::String(uri.clone()));
    }
}

#[test]
fn model_without_schema_entry_offers_the_scaffold() {
    let ctx = ctx();
    let uri = model_uri("models/staging/stg_stripe_payments.sql");
    let actions = features::code_actions(&ctx, None, &uri, Range::default(), None);
    let titles = titles(&actions);
    assert!(
        titles.contains(&"Extract schema YAML".to_string()),
        "{titles:?}"
    );
    assert!(
        !actions.iter().any(|a| a.command.command == "dbt.goToTests"),
        "{titles:?}"
    );
}

#[test]
fn kind_filter_yields_nothing() {
    let ctx = ctx();
    let uri = model_uri("models/customers.sql");
    let only = vec!["quickfix".to_string()];
    let actions = features::code_actions(&ctx, None, &uri, Range::default(), Some(&only));
    assert_eq!(titles(&actions), Vec::<String>::new());
}

#[test]
fn ref_token_prepends_the_target_schema_action() {
    let ctx = ctx();
    let uri = "file:///tmp/model.sql".to_string();
    let text = "select * from {{ ref('customers') }}";
    let mut docs = Documents::default();
    docs.open(uri.clone(), text, &ctx.dialect);
    let range = Range::zero_width(Position {
        line: 0,
        character: 24,
    });
    let actions = features::code_actions(&ctx, docs.get(&uri), &uri, range, None);
    assert_eq!(actions[0].title, "Open schema of 'customers'");
    assert_eq!(actions[0].command.command, "dbt.openSchema");
    let arg = serde_json::to_value(&actions[0].command.arguments[0]).unwrap();
    assert!(
        arg["uri"]
            .as_str()
            .is_some_and(|u| u.ends_with("customers.sql")),
        "{arg}"
    );
}

#[test]
fn yaml_entry_offers_model_and_tests_jumps() {
    let ctx = ctx();
    let entry_line = ctx
        .model_detail_map
        .get("customers")
        .expect("customers")
        .schema_range
        .start
        .line;
    let uri = model_uri("models/schema.yml");
    let range = Range::zero_width(Position {
        line: entry_line,
        character: 0,
    });
    let actions = features::code_actions(&ctx, None, &uri, range, None);
    assert_eq!(
        titles(&actions),
        vec![
            "Open model 'customers'".to_string(),
            "Go to tests of 'customers'".to_string()
        ]
    );
    assert_eq!(actions[0].command.command, "dbt.openModel");
    let arg = serde_json::to_value(&actions[0].command.arguments[0]).unwrap();
    assert!(
        arg["uri"]
            .as_str()
            .is_some_and(|u| u.ends_with("customers.sql")),
        "{arg}"
    );
}
