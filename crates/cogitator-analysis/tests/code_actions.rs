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

/// Every action in these tests is command-only; unwrap accordingly.
fn cmd(a: &cogitator_lsp::CodeAction) -> &cogitator_lsp::Command {
    a.command.as_ref().expect("command-only action")
}

#[test]
fn model_file_offers_the_lens_command_set() {
    let ctx = ctx();
    let uri = model_uri("models/customers.sql");
    let actions = features::code_actions(&ctx, None, &uri, Range::default(), None, None, &[]);
    let titles = titles(&actions);
    assert!(titles.contains(&"Go to schema".to_string()), "{titles:?}");
    assert!(titles.contains(&"dbt run".to_string()), "{titles:?}");
    // customers has a schema entry with tests: the goToTests tail, not the
    // extract scaffold
    assert!(
        actions.iter().any(|a| cmd(a).command == "dbt.goToTests"),
        "{titles:?}"
    );
    assert!(
        !titles.contains(&"Extract schema YAML".to_string()),
        "{titles:?}"
    );
    for a in &actions {
        assert!(cmd(a).command.starts_with("dbt."), "{}", cmd(a).command);
        let arg = serde_json::to_value(&cmd(a).arguments[0]).unwrap();
        assert_eq!(arg["uri"], serde_json::Value::String(uri.clone()));
    }
}

#[test]
fn model_without_schema_entry_offers_the_scaffold() {
    let ctx = ctx();
    let uri = model_uri("models/staging/stg_stripe_payments.sql");
    let actions = features::code_actions(&ctx, None, &uri, Range::default(), None, None, &[]);
    let titles = titles(&actions);
    assert!(
        titles.contains(&"Extract schema YAML".to_string()),
        "{titles:?}"
    );
    assert!(
        !actions.iter().any(|a| cmd(a).command == "dbt.goToTests"),
        "{titles:?}"
    );
}

#[test]
fn kind_filter_yields_nothing() {
    let ctx = ctx();
    let uri = model_uri("models/customers.sql");
    let only = vec!["quickfix".to_string()];
    let actions =
        features::code_actions(&ctx, None, &uri, Range::default(), Some(&only), None, &[]);
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
    let actions = features::code_actions(&ctx, docs.get(&uri), &uri, range, None, None, &[]);
    assert_eq!(actions[0].title, "Open schema of 'customers'");
    assert_eq!(cmd(&actions[0]).command, "dbt.openSchema");
    let arg = serde_json::to_value(&cmd(&actions[0]).arguments[0]).unwrap();
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
    let actions = features::code_actions(&ctx, None, &uri, range, None, None, &[]);
    assert_eq!(
        titles(&actions),
        vec![
            "Open model 'customers'".to_string(),
            "Go to tests of 'customers'".to_string()
        ]
    );
    assert_eq!(cmd(&actions[0]).command, "dbt.openModel");
    let arg = serde_json::to_value(&cmd(&actions[0]).arguments[0]).unwrap();
    assert!(
        arg["uri"]
            .as_str()
            .is_some_and(|u| u.ends_with("customers.sql")),
        "{arg}"
    );
}

#[test]
fn profiles_yaml_offers_target_switch_actions() {
    use cogitator_analysis::features::TargetChoice;
    let ctx = ctx();
    let choices = vec![
        TargetChoice {
            profile: "jaffle_shop".into(),
            target: "dev".into(),
            active: true,
        },
        TargetChoice {
            profile: "jaffle_shop".into(),
            target: "prod".into(),
            active: false,
        },
    ];
    let uri = "file:///home/me/.dbt/profiles.yml";
    let actions =
        features::code_actions(&ctx, None, uri, Range::default(), None, Some(&choices), &[]);
    assert_eq!(
        titles(&actions),
        vec![
            "Switch target: jaffle_shop/dev (active)".to_string(),
            "Switch target: jaffle_shop/prod".to_string()
        ]
    );
    assert_eq!(cmd(&actions[1]).command, "dbt.setTarget");
    let arg = serde_json::to_value(&cmd(&actions[1]).arguments[0]).unwrap();
    assert_eq!(arg["target"], "prod");
    assert_eq!(arg["profile"], "jaffle_shop");
}

#[test]
fn a_ref_to_no_model_offers_to_create_it_beside_the_referencing_one() {
    let ctx = ctx();
    let uri = model_uri("models/probe.sql");
    let text = "select * from {{ ref('brand_new_model') }}";
    let mut docs = Documents::default();
    docs.open(uri.clone(), text, &ctx.dialect);
    let range = Range::zero_width(Position {
        line: 0,
        character: 25,
    });
    let read = |_: &std::path::Path| -> Option<String> { None };
    let actions =
        features::define_missing_actions(&ctx, docs.get(&uri).unwrap(), &uri, range, &read);
    assert_eq!(
        titles(&actions),
        vec!["Create model 'brand_new_model'".to_string()]
    );
    let wire = serde_json::to_value(actions[0].edit.as_ref().unwrap()).unwrap();
    assert_eq!(
        wire["documentChanges"][0]["uri"].as_str(),
        Some(model_uri("models/brand_new_model.sql").as_str())
    );

    // a ref to a model that exists offers nothing of the kind
    let known = "select * from {{ ref('customers') }}";
    docs.open(uri.clone(), known, &ctx.dialect);
    let range = Range::zero_width(Position {
        line: 0,
        character: 24,
    });
    assert!(
        features::define_missing_actions(&ctx, docs.get(&uri).unwrap(), &uri, range, &read)
            .is_empty()
    );
}

#[test]
fn an_unknown_var_offers_to_define_it_in_dbt_project_yml() {
    let ctx = ctx();
    let uri = model_uri("models/probe.sql");
    let text = "select {{ var('brand_new_var') }} as v";
    let mut docs = Documents::default();
    docs.open(uri.clone(), text, &ctx.dialect);
    let range = Range::zero_width(Position {
        line: 0,
        character: 18,
    });
    let read = |path: &std::path::Path| std::fs::read_to_string(path).ok();
    let actions =
        features::define_missing_actions(&ctx, docs.get(&uri).unwrap(), &uri, range, &read);
    assert_eq!(
        titles(&actions),
        vec!["Define var 'brand_new_var' in dbt_project.yml".to_string()]
    );
    let wire = serde_json::to_value(actions[0].edit.as_ref().unwrap()).unwrap();
    let change = &wire["documentChanges"][0];
    assert_eq!(
        change["textDocument"]["uri"].as_str(),
        Some(model_uri("dbt_project.yml").as_str())
    );
    // at the top level of vars:, after its last line (the fixture's
    // project-scoped jaffle_shop block)
    let edit = &change["edits"][0];
    assert_eq!(edit["newText"], "  brand_new_var: null\n");
    let project = std::fs::read_to_string(fixture_root().join("dbt_project.yml")).unwrap();
    let line = edit["range"]["start"]["line"].as_u64().unwrap() as usize;
    let before: Vec<&str> = project.lines().take(line).collect();
    assert_eq!(before.last(), Some(&"    jaffle_number: 1"));
}

fn open(text: &str) -> (cogitator_analysis::DbtContext, Documents, String) {
    let ctx = ctx();
    let uri = model_uri("models/probe.sql");
    let mut docs = Documents::default();
    docs.open(uri.clone(), text, &ctx.dialect);
    (ctx, docs, uri)
}

fn at(character: u32) -> Range {
    Range::zero_width(Position { line: 0, character })
}

fn new_text(action: &cogitator_lsp::CodeAction) -> String {
    let wire = serde_json::to_value(action.edit.as_ref().unwrap()).unwrap();
    wire["documentChanges"][0]["edits"][0]["newText"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn a_qualified_star_expands_to_the_relations_known_columns() {
    let text = "select o.*, c.customer_id from {{ ref('stg_orders') }} o \
                join {{ ref('stg_customers') }} c on o.order_id = c.customer_id";
    let (ctx, docs, uri) = open(text);
    for cursor in [7, 8, 9] {
        let action = features::expand_star_action(&ctx, docs.get(&uri).unwrap(), &uri, at(cursor))
            .unwrap_or_else(|| panic!("an action at {cursor}"));
        assert_eq!(action.title, "Expand o.* to 2 column(s)");
        assert_eq!(action.kind, Some("refactor.rewrite"));
        assert_eq!(new_text(&action), "o.order_id, o.status");
    }
    // on another select item: nothing
    assert!(features::expand_star_action(&ctx, docs.get(&uri).unwrap(), &uri, at(14)).is_none());
}

#[test]
fn a_bare_star_expands_only_as_a_select_item_of_one_relation() {
    let (ctx, docs, uri) = open("select * from {{ ref('stg_orders') }}");
    let action =
        features::expand_star_action(&ctx, docs.get(&uri).unwrap(), &uri, at(7)).expect("action");
    assert_eq!(new_text(&action), "order_id, status");

    let (ctx, docs, uri) = open("select count(*) as n from {{ ref('stg_orders') }}");
    assert!(features::expand_star_action(&ctx, docs.get(&uri).unwrap(), &uri, at(13)).is_none());

    // a relation whose columns nobody documents: no expansion
    let (ctx, docs, uri) = open("select * from {{ ref('stg_stripe_payments') }}");
    assert!(features::expand_star_action(&ctx, docs.get(&uri).unwrap(), &uri, at(7)).is_none());
}

#[test]
fn schema_extraction_is_an_edit_too() {
    let ctx = ctx();
    let uri = model_uri("models/staging/stg_stripe_payments.sql");
    let extract = features::extract_model_yaml(&ctx, &uri, None).expect("an extraction");
    let action = features::extract_yaml_action("stg_stripe_payments", &extract);
    assert_eq!(
        action.title,
        "Extract schema YAML of 'stg_stripe_payments' to stg_stripe_payments.yml"
    );
    assert_eq!(action.kind, Some("refactor.extract"));
    let wire = serde_json::to_value(action.edit.as_ref().unwrap()).unwrap();
    assert_eq!(wire["documentChanges"][0]["kind"], "create");
    assert!(wire["documentChanges"][1]["edits"][0]["newText"]
        .as_str()
        .unwrap()
        .contains("- name: stg_stripe_payments"));
}

/// The text after applying `edits` (byte columns, as the server sends).
fn applied(text: &str, edits: &serde_json::Value) -> String {
    let starts: Vec<usize> = std::iter::once(0)
        .chain(text.match_indices('\n').map(|(i, _)| i + 1))
        .collect();
    let offset = |p: &serde_json::Value| {
        starts[p["line"].as_u64().unwrap() as usize] + p["character"].as_u64().unwrap() as usize
    };
    let mut edits: Vec<(usize, usize, String)> = edits
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                offset(&e["range"]["start"]),
                offset(&e["range"]["end"]),
                e["newText"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    edits.sort_by_key(|e| std::cmp::Reverse(e.0));
    let mut out = text.to_string();
    for (start, end, new) in edits {
        out.replace_range(start..end, &new);
    }
    out
}

fn extract(text: &str, needle: &str) -> Option<(String, String, String)> {
    let (ctx, docs, uri) = open_lines(text);
    let (line, column) = text
        .lines()
        .enumerate()
        .find_map(|(i, l)| l.find(needle).map(|c| (i, c)))
        .unwrap();
    let range = Range::zero_width(Position {
        line: line as u32,
        character: column as u32,
    });
    let action = features::extract_cte_action(&ctx, docs.get(&uri).unwrap(), &uri, range)?;
    let wire = serde_json::to_value(action.edit.as_ref().unwrap()).unwrap();
    let changes = &wire["documentChanges"];
    let created = changes[0]["uri"].as_str().unwrap().to_string();
    let body = changes[1]["edits"][0]["newText"]
        .as_str()
        .unwrap()
        .to_string();
    let rest = applied(text, &changes[2]["edits"]);
    assert_eq!(action.kind, Some("refactor.extract"));
    Some((created, body, rest))
}

fn open_lines(text: &str) -> (cogitator_analysis::DbtContext, Documents, String) {
    open(text)
}

#[test]
fn the_first_of_two_ctes_moves_to_a_model_and_is_ref_d() {
    let text = "with recent as (\n    select * from {{ ref('stg_orders') }}\n),\n\nfinal as (\n    select r.order_id from recent r\n)\n\nselect * from final\n";
    let (created, body, rest) = extract(text, "recent as").expect("an action");
    assert_eq!(created, model_uri("models/recent.sql"));
    assert_eq!(body, "select * from {{ ref('stg_orders') }}\n");
    assert_eq!(
        rest,
        "with final as (\n    select r.order_id from {{ ref('recent') }} r\n)\n\nselect * from final\n"
    );
}

#[test]
fn an_only_cte_takes_its_with_along_and_its_use_keeps_the_name() {
    let text = "with base as (\n    select 1 as id\n)\nselect base.id from base\n";
    let (_, body, rest) = extract(text, "base.id").expect("an action on a use");
    assert_eq!(body, "select 1 as id\n");
    assert_eq!(rest, "select base.id from {{ ref('base') }} as base\n");
}

#[test]
fn the_last_of_several_ctes_takes_the_comma_before_it() {
    let text = "with a as (\n    select 1 as id\n), b as (\n    select 2 as id\n)\nselect * from a join b on a.id = b.id\n";
    let (_, _, rest) = extract(text, "b as").expect("an action");
    assert_eq!(
        rest,
        "with a as (\n    select 1 as id\n)\nselect * from a join {{ ref('b') }} as b on a.id = b.id\n"
    );
}

#[test]
fn a_cte_that_would_not_stand_alone_is_not_extracted() {
    // reads another CTE of the file
    let text =
        "with a as (\n    select 1 as id\n), b as (\n    select id from a\n)\nselect * from b\n";
    assert!(extract(text, "b as").is_none());
    // a model of that name exists
    let text = "with orders as (\n    select 1 as id\n)\nselect * from orders\n";
    assert!(extract(text, "orders as").is_none());
}

#[test]
fn a_packages_file_naming_missing_packages_offers_dbt_deps() {
    let mut ctx = ctx();
    ctx.declared_packages.push("not_installed_pkg".into());
    let uri = model_uri("packages.yml");
    let actions = features::code_actions(&ctx, None, &uri, Range::default(), None, None, &[]);
    let deps = actions
        .iter()
        .find(|a| a.command.as_ref().is_some_and(|c| c.command == "dbt.deps"))
        .unwrap_or_else(|| panic!("{:?}", titles(&actions)));
    assert_eq!(deps.title, "Run dbt deps (1 package(s) missing)");
    assert_eq!(deps.kind, Some("quickfix"));
    // a quickfix request gets it too; elsewhere there is none
    let only = ["quickfix".to_string()];
    let fixes = features::code_actions(&ctx, None, &uri, Range::default(), Some(&only), None, &[]);
    assert_eq!(titles(&fixes), vec![deps.title.clone()]);
    let model = model_uri("models/customers.sql");
    let elsewhere = features::code_actions(&ctx, None, &model, Range::default(), None, None, &[]);
    assert!(elsewhere.iter().all(|a| cmd(a).command != "dbt.deps"));
}
