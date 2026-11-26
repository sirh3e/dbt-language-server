use std::collections::HashMap;

use super::*;

fn inputs() -> PreviewInputs {
    PreviewInputs {
        macro_sources: Vec::new(),
        model_relations: HashMap::from([("orders".to_string(), "db.main.orders".to_string())]),
        source_relations: HashMap::new(),
        vars: HashMap::new(),
        target_name: "dev".to_string(),
        is_incremental: false,
        this: None,
        env: HashMap::new(),
    }
}

const OLD: &str = "select id, amount from {{ ref('orders') }}\n";
const NEW: &str = "select id, amount, status from {{ ref('orders') }}\n";

fn request<'a>(what: DiffWhat, old: OldSide, dialect: &'a Dialect) -> DiffRequest<'a> {
    DiffRequest {
        model: "m",
        rev: "HEAD",
        what,
        old,
        current: NEW,
        dialect,
    }
}

#[test]
fn sql_view_is_a_unified_diff_of_the_templates() {
    let dialect = Dialect::default();
    let diff = diff_at(
        request(DiffWhat::Sql, OldSide::Template(OLD.into()), &dialect),
        &inputs(),
    )
    .unwrap();
    assert!(!diff.identical);
    assert!(diff.diff.contains("--- m @ HEAD (sql)"), "{}", diff.diff);
    assert!(
        diff.diff.contains("+select id, amount, status"),
        "{}",
        diff.diff
    );
}

#[test]
fn compiled_view_renders_both_sides() {
    let dialect = Dialect::default();
    let diff = diff_at(
        request(DiffWhat::Compiled, OldSide::Template(OLD.into()), &dialect),
        &inputs(),
    )
    .unwrap();
    assert!(diff.before.contains("db.main.orders"), "{}", diff.before);
    assert!(diff.note.is_some());
    let against_dbt = diff_at(
        request(
            DiffWhat::Compiled,
            OldSide::Compiled("select id, amount, status from db.main.orders\n".into()),
            &dialect,
        ),
        &inputs(),
    )
    .unwrap();
    assert!(against_dbt.identical, "{}", against_dbt.diff);
}

#[test]
fn lineage_view_names_the_new_column() {
    let dialect = Dialect::default();
    let diff = diff_at(
        request(DiffWhat::Lineage, OldSide::Template(OLD.into()), &dialect),
        &inputs(),
    )
    .unwrap();
    let columns = diff.columns.clone().unwrap();
    assert_eq!(columns.added, ["status"], "{columns:?}");
    assert!(shown_text(&diff).contains("-- + column status"));
}

#[test]
fn views_parse_with_did_you_mean() {
    assert_eq!(DiffWhat::parse(None).unwrap(), DiffWhat::Sql);
    let message = DiffWhat::parse(Some("compild")).unwrap_err().to_message();
    assert!(message.contains("compiled"), "{message}");
}
