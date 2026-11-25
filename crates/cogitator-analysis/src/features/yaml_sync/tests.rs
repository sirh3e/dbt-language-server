//! The planner over small projects on disk: what each model's entry
//! becomes, where descriptions come from, and that a second sync after
//! applying changes nothing.

use pretty_assertions::assert_eq;

use super::test_project::{base, Project};
use super::*;

fn summary<'p>(plan: &'p SyncPlan, model: &str) -> &'p ModelSummary {
    plan.models
        .iter()
        .find(|m| m.model == model)
        .unwrap_or_else(|| panic!("no summary for {model}: {plan:#?}"))
}

fn inherited(s: &ModelSummary) -> Vec<(String, String)> {
    s.inherited
        .iter()
        .map(|i| {
            let from = match &i.from {
                SourceRef::Model { name, column } => format!("{name}.{column}"),
                SourceRef::Source {
                    source,
                    table,
                    column,
                } => format!("source {source}.{table}.{column}"),
                SourceRef::Unknown { name, column } => format!("? {name}.{column}"),
            };
            (i.column.clone(), format!("{from} via {:?}", i.via))
        })
        .collect()
}

#[test]
fn a_model_without_an_entry_gets_one_with_its_columns_in_order() {
    let project = Project::new(&base());
    let plan = project.plan(&SyncOptions::default());
    let orders = summary(&plan, "orders");
    assert!(orders.created_entry, "{orders:#?}");
    assert_eq!(orders.added, ["id", "amount", "state", "customer"]);
    let file = plan
        .files
        .iter()
        .find(|f| f.path.ends_with("marts/orders.yml"))
        .expect("created file");
    assert!(file.created);
    insta::assert_snapshot!(file.new_text);
}

#[test]
fn descriptions_follow_pass_through_columns_and_nothing_else() {
    let project = Project::new(&base());
    let plan = project.plan(&SyncOptions::default());
    // through `select *` and two CTEs, past the undocumented stg_orders.id
    assert_eq!(
        inherited(summary(&plan, "orders")),
        [
            (
                "id".to_string(),
                "source raw.orders.id via [\"stg_orders\"]".to_string()
            ),
            (
                "customer".to_string(),
                "source raw.orders.customer via [\"stg_orders\"]".to_string()
            ),
        ]
    );
    // `amount + 1 as amount` and `status as state` inherit nothing
    assert_eq!(
        summary(&plan, "orders").undocumented,
        ["amount", "state"],
        "{plan:#?}"
    );
    let stg = summary(&plan, "stg_orders");
    assert_eq!(
        inherited(stg),
        [
            ("id".to_string(), "source raw.orders.id via []".to_string()),
            (
                "amount".to_string(),
                "source raw.orders.amount via []".to_string()
            ),
            (
                "customer".to_string(),
                "source raw.orders.customer via []".to_string()
            ),
        ]
    );
    assert_eq!(stg.added, ["amount", "customer"]);
}

#[test]
fn a_written_description_is_kept() {
    let project = Project::new(&base());
    project.sync(&SyncOptions::default());
    let text = project.read("models/staging/stg_orders.yml");
    assert!(
        text.contains("description: Where the order stands.\n"),
        "{text}"
    );
    assert_eq!(text.matches("description:").count(), 4, "{text}");
}

#[test]
fn stale_columns_are_reported_and_pruned_only_when_asked() {
    let mut files = base();
    files[2].1 = "version: 2\nmodels:\n  - name: stg_orders\n    columns:\n      - name: id\n        description: Key.\n      - name: legacy\n        description: Gone.\n        data_tests: [not_null]\n      - name: status\n        description: S.\n";
    let project = Project::new(&files);
    let plan = project.plan(&SyncOptions::default());
    let stg = summary(&plan, "stg_orders");
    assert_eq!(stg.stale, ["legacy"]);
    assert!(stg.pruned.is_empty());
    let plan = project.sync(&SyncOptions {
        prune: true,
        ..SyncOptions::default()
    });
    assert_eq!(summary(&plan, "stg_orders").pruned, ["legacy"]);
    let text = project.read("models/staging/stg_orders.yml");
    assert!(
        !text.contains("legacy") && !text.contains("Gone."),
        "{text}"
    );
    assert!(text.contains("      - name: status\n        description: S.\n"));
}

const BRANCHY: &str = "{% if var('x', true) %}select id from {{ ref('stg_orders') }}\
                       {% else %}select 1 as id{% endif %}\n";

#[test]
fn a_model_with_unclear_lineage_is_skipped_with_the_reason() {
    let mut files = base();
    files.push(("models/marts/branchy.sql", BRANCHY));
    let project = Project::new(&files);
    let plan = project.plan(&SyncOptions::default());
    let branchy = summary(&plan, "branchy");
    assert_eq!(branchy.changes, 0);
    assert!(
        branchy.skipped[0].reason.contains("lineage is unclear"),
        "{branchy:#?}"
    );
    assert!(!plan.files.iter().any(|f| f.path.ends_with("branchy.yml")));
}

#[test]
fn a_column_downstream_of_unclear_lineage_says_why() {
    let mut files = base();
    files.push(("models/marts/branchy.sql", BRANCHY));
    files.push((
        "models/marts/after.sql",
        "select id from {{ ref('branchy') }}\n",
    ));
    let project = Project::new(&files);
    let plan = project.plan(&SyncOptions {
        models: vec!["after".into()],
        ..SyncOptions::default()
    });
    let after = summary(&plan, "after");
    assert_eq!(after.added, ["id"]);
    assert!(after.inherited.is_empty());
    assert_eq!(after.skipped.len(), 1, "{after:#?}");
    assert_eq!(after.skipped[0].column.as_deref(), Some("id"));
    assert!(after.skipped[0].reason.contains("'branchy'"));
}

#[test]
fn a_second_sync_after_applying_changes_nothing() {
    for options in [
        SyncOptions::default(),
        SyncOptions {
            prune: true,
            reorder: true,
            inherit_types: true,
            ..SyncOptions::default()
        },
        SyncOptions {
            new_entries: NewEntries::PerFolder,
            ..SyncOptions::default()
        },
    ] {
        let project = Project::new(&base());
        let first = project.sync(&options);
        assert!(first.changes > 0);
        let second = project.plan(&options);
        assert_eq!(second.changes, 0, "{options:?}: {second:#?}");
        assert!(second.files.is_empty(), "{:#?}", second.files);
    }
}

#[test]
fn per_folder_entries_share_one_file() {
    let mut files = base();
    files.push((
        "models/marts/items.sql",
        "select id from {{ ref('orders') }}\n",
    ));
    let project = Project::new(&files);
    let plan = project.sync(&SyncOptions {
        new_entries: NewEntries::PerFolder,
        ..SyncOptions::default()
    });
    assert!(summary(&plan, "items").created_entry);
    let text = project.read("models/marts/_marts__models.yml");
    insta::assert_snapshot!(text);
    // items.id inherits through orders.id, which the same sync documents
    assert_eq!(
        inherited(summary(&plan, "items")),
        [(
            "id".to_string(),
            "source raw.orders.id via [\"orders\", \"stg_orders\"]".to_string()
        )]
    );
}

#[test]
fn only_the_models_asked_for() {
    let project = Project::new(&base());
    let plan = project.plan(&SyncOptions {
        models: vec!["stg_orders".into(), "nope".into()],
        ..SyncOptions::default()
    });
    let names: Vec<&str> = plan.models.iter().map(|m| m.model.as_str()).collect();
    assert_eq!(names, ["stg_orders", "nope"]);
    assert!(summary(&plan, "nope").skipped[0]
        .reason
        .contains("not a SQL model"));
    assert_eq!(plan.files.len(), 1);
}

#[test]
fn proven_types_fill_data_type_only_with_the_flag() {
    let files = vec![
        ("models/a.sql", "select cast(1 as integer) as n, 'x' as s\n"),
        (
            "models/a.yml",
            "models:\n  - name: a\n    columns:\n      - name: n\n        description: N.\n",
        ),
    ];
    let project = Project::new(&files);
    let plan = project.plan(&SyncOptions::default());
    assert!(summary(&plan, "a").typed.is_empty());
    let plan = project.sync(&SyncOptions {
        inherit_types: true,
        ..SyncOptions::default()
    });
    let a = summary(&plan, "a");
    let typed: Vec<&str> = a.typed.iter().map(|t| t.column.as_str()).collect();
    assert!(typed.contains(&"n"), "{a:#?}");
    let text = project.read("models/a.yml");
    assert!(
        text.contains("      - name: n\n        data_type: "),
        "{text}"
    );
}

#[test]
fn reorder_follows_the_sql_and_is_off_by_default() {
    let files = vec![
        ("models/a.sql", "select 1 as x, 2 as y, 3 as z\n"),
        (
            "models/a.yml",
            "models:\n  - name: a\n    columns:\n      - name: z\n        description: Z.\n      # why y\n      - name: y\n        description: Y.\n      - name: x\n        description: X.\n",
        ),
    ];
    let project = Project::new(&files);
    assert_eq!(project.plan(&SyncOptions::default()).changes, 0);
    let plan = project.sync(&SyncOptions {
        reorder: true,
        ..SyncOptions::default()
    });
    assert!(summary(&plan, "a").reordered);
    assert_eq!(
        project.read("models/a.yml"),
        "models:\n  - name: a\n    columns:\n      - name: x\n        description: X.\n      # why y\n      - name: y\n        description: Y.\n      - name: z\n        description: Z.\n"
    );
}

#[test]
fn the_plan_is_one_workspace_edit() {
    let project = Project::new(&base());
    let plan = project.plan(&SyncOptions::default());
    let edit = plan.workspace_edit();
    let json = serde_json::to_value(&edit).expect("json");
    let changes = json["documentChanges"].as_array().expect("changes");
    assert_eq!(changes[0]["kind"], "create");
    assert!(changes.len() >= 3, "{json}");
}
