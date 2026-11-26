use std::sync::Arc;

use cogitator_dialects::Dialect;
use cogitator_parser::{DocumentParser, JinjaSqlParser};
use serde_json::json;

use super::*;

fn columns() -> Vec<String> {
    [
        "KIND",
        "VALUE",
        "VALUE2",
        "N",
        "DISTINCT_COUNT",
        "NULL_COUNT",
    ]
    .map(String::from)
    .to_vec()
}

fn rows() -> Vec<Vec<Value>> {
    vec![
        vec![
            json!("top"),
            json!("paid"),
            json!(null),
            json!(3),
            json!(null),
            json!(null),
        ],
        vec![
            json!("stats"),
            json!("a"),
            json!("z"),
            json!("10"),
            json!(4),
            json!(2),
        ],
        vec![
            json!("top"),
            json!("open"),
            json!(null),
            json!(5),
            json!(null),
            json!(null),
        ],
    ]
}

#[test]
fn rows_read_case_insensitively_and_sort_top_values() {
    let key = ProfileKey::new("orders", "Status");
    let profile = profile_of(
        &key,
        "db.main.orders",
        "duckdb",
        "",
        "q".into(),
        &columns(),
        &rows(),
    )
    .unwrap();
    assert_eq!((profile.rows, profile.distinct, profile.nulls), (10, 4, 2));
    assert_eq!(profile.null_rate, 0.2);
    assert_eq!(profile.top[0].value, "open");
    assert_eq!(profile.column, "status");
    let hover = hover_markdown(&profile);
    assert!(hover.contains("distinct 4 · null 20.0%"), "{hover}");
    assert!(hover.contains("min `a` · max `z`"), "{hover}");
    assert!(hover.contains("top values █▅"), "{hover}");
    assert!(hover.contains("no dbt run recorded"), "{hover}");
}

#[test]
fn missing_statistics_or_columns_are_guidance() {
    let key = ProfileKey::new("m", "c");
    assert!(profile_of(&key, "r", "e", "", String::new(), &columns(), &[]).is_err());
    assert!(profile_of(&key, "r", "e", "", String::new(), &["x".into()], &rows()).is_err());
}

#[test]
fn the_target_is_an_output_column_of_the_model() {
    let text = "select id, status from {{ ref('stg_orders') }}";
    let dialect = Dialect::new("duckdb");
    let doc = crate::Document {
        uri: Arc::from("file:///p/models/orders.sql"),
        text: Arc::from(text),
        parsed: JinjaSqlParser.parse(text, &dialect).unwrap(),
        generation: 1,
    };
    let lineage = crate::lineage::column_lineage(&doc.parsed, &dialect);
    let ctx = DbtContext::default();
    let at = |needle: &str| Position {
        line: 0,
        character: text.find(needle).unwrap() as u32 + 1,
    };
    let key = profile_target(&ctx, "orders", &doc, lineage.as_ref(), at("status")).unwrap();
    assert_eq!(key, ProfileKey::new("orders", "status"));
    assert!(profile_target(&ctx, "orders", &doc, lineage.as_ref(), at("select")).is_none());
    assert!(profile_target(&ctx, "orders", &doc, None, at("status")).is_none());
}
