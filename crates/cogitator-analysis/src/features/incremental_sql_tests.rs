use std::collections::HashMap;

use pretty_assertions::assert_eq;
use rstest::rstest;

use super::*;
use crate::Documents;

fn inputs() -> PreviewInputs {
    PreviewInputs {
        macro_sources: Vec::new(),
        model_relations: HashMap::from([
            ("orders".to_string(), "shop.main.orders".to_string()),
            ("stg_orders".to_string(), "shop.main.stg_orders".to_string()),
        ]),
        source_relations: HashMap::new(),
        vars: HashMap::new(),
        target_name: "dev".to_string(),
        is_incremental: false,
        this: None,
        env: HashMap::new(),
    }
}

fn model(config: &str) -> String {
    format!(
        "{{{{ config({config}) }}}}\nselect order_id, status, updated_at\nfrom {{{{ ref('stg_orders') }}}}\n\
         {{% if is_incremental() %}}\nwhere updated_at > (select max(updated_at) from {{{{ this }}}})\n{{% endif %}}\n"
    )
}

fn cols() -> Vec<String> {
    vec!["order_id".into(), "status".into(), "updated_at".into()]
}

fn run(config: &str, adapter: &str, strategy: Option<&str>) -> Result<IncrementalSql, Guidance> {
    incremental_sql(
        "orders",
        &model(config),
        adapter,
        &cols(),
        &mut inputs(),
        strategy,
    )
}

#[rstest]
#[case::config_wins(
    "materialized='incremental', incremental_strategy='merge'",
    "duckdb",
    "merge",
    "config"
)]
#[case::postgres_with_a_key(
    "materialized='incremental', unique_key='order_id'",
    "postgres",
    "delete+insert",
    "adapter default"
)]
#[case::postgres_without("materialized='incremental'", "postgres", "append", "adapter default")]
#[case::snowflake("materialized='incremental'", "snowflake", "merge", "adapter default")]
fn the_strategy_is_the_configs_or_the_adapters(
    #[case] config: &str,
    #[case] adapter: &str,
    #[case] strategy: &str,
    #[case] source: &str,
) {
    let answer = run(config, adapter, None).unwrap();
    assert_eq!(
        (answer.strategy.as_str(), answer.strategy_source.as_str()),
        (strategy, source)
    );
}

#[test]
fn the_new_rows_are_staged_with_is_incremental_true() {
    let answer = run("materialized='incremental'", "duckdb", None).unwrap();
    assert_eq!(
        answer.steps[0].sql,
        "create temporary table shop.main.orders__dbt_tmp as (\n    select order_id, status, updated_at\n    \
         from shop.main.stg_orders\n\n    where updated_at > (select max(updated_at) from shop.main.orders)\n);\n"
    );
    assert!(
        !answer.full_refresh.contains("max(updated_at)"),
        "{}",
        answer.full_refresh
    );
}

#[test]
fn merge_matches_on_the_key_with_the_predicates() {
    let answer = run(
        "materialized='incremental', incremental_strategy='merge', unique_key='order_id', \
         incremental_predicates=[\"DBT_INTERNAL_DEST.updated_at > dateadd(day, -7, current_date)\"], \
         merge_exclude_columns=['updated_at']",
        "snowflake",
        None,
    )
    .unwrap();
    assert_eq!(
        answer.steps[1].sql,
        "merge into shop.main.orders as DBT_INTERNAL_DEST\nusing shop.main.orders__dbt_tmp as DBT_INTERNAL_SOURCE\n\
         on DBT_INTERNAL_SOURCE.order_id = DBT_INTERNAL_DEST.order_id\n    \
         and DBT_INTERNAL_DEST.updated_at > dateadd(day, -7, current_date)\n\
         when matched then update set\n    order_id = DBT_INTERNAL_SOURCE.order_id,\n    \
         status = DBT_INTERNAL_SOURCE.status\n\
         when not matched then insert\n    (order_id, status, updated_at)\nvalues\n    \
         (DBT_INTERNAL_SOURCE.order_id, DBT_INTERNAL_SOURCE.status, DBT_INTERNAL_SOURCE.updated_at);\n"
    );
}

#[test]
fn delete_insert_deletes_by_the_key_then_inserts() {
    let answer = run(
        "materialized='incremental', unique_key='order_id'",
        "postgres",
        None,
    )
    .unwrap();
    let titles: Vec<_> = answer.steps.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(
        titles,
        vec![
            "stage the new rows (the model with is_incremental() true)",
            "delete the rows the new ones replace (by order_id)",
            "insert the new rows"
        ]
    );
    assert_eq!(
        answer.steps[1].sql,
        "delete from shop.main.orders\nwhere (order_id) in (\n    select (order_id)\n    \
         from shop.main.orders__dbt_tmp\n);\n"
    );
}

#[test]
fn a_composite_key_deletes_with_using() {
    let answer = run(
        "materialized='incremental', incremental_strategy='delete+insert', unique_key=['order_id', 'status']",
        "duckdb",
        None,
    )
    .unwrap();
    assert!(answer.steps[1]
        .sql
        .starts_with("delete from shop.main.orders\nusing shop.main.orders__dbt_tmp\n"));
}

#[rstest]
#[case::bigquery("bigquery", "when not matched by source")]
#[case::databricks(
    "databricks",
    "insert overwrite table shop.main.orders\npartition (order_date)"
)]
#[case::duckdb("duckdb", "where order_date in (")]
fn insert_overwrite_follows_the_adapter(#[case] adapter: &str, #[case] shape: &str) {
    let answer = run(
        "materialized='incremental', incremental_strategy='insert_overwrite', \
         partition_by={'field': 'order_date', 'data_type': 'date'}",
        adapter,
        None,
    )
    .unwrap();
    assert!(answer.text.contains(shape), "{}", answer.text);
}

#[test]
fn microbatch_runs_per_batch_of_the_event_time() {
    let answer = run(
        "materialized='incremental', incremental_strategy='microbatch', event_time='updated_at', batch_size='hour'",
        "duckdb",
        None,
    )
    .unwrap();
    assert!(answer.steps[0].title.starts_with("for each hour batch"));
    assert!(answer.steps[1]
        .sql
        .contains("where updated_at >= '<batch start>'"));
}

#[rstest]
#[case::delete_insert_without_key(
    "materialized='incremental'",
    Some("delete+insert"),
    "needs a unique_key"
)]
#[case::overwrite_without_partition(
    "materialized='incremental'",
    Some("insert_overwrite"),
    "needs a partition_by"
)]
#[case::microbatch_without_event_time(
    "materialized='incremental'",
    Some("microbatch"),
    "needs an event_time"
)]
#[case::unknown(
    "materialized='incremental'",
    Some("mergee"),
    "not an incremental strategy"
)]
fn what_a_strategy_lacks_is_said(
    #[case] config: &str,
    #[case] strategy: Option<&str>,
    #[case] says: &str,
) {
    let err = run(config, "duckdb", strategy).unwrap_err();
    assert!(err.headline.contains(says), "{}", err.headline);
}

#[test]
fn the_override_is_used_and_said() {
    let answer = run(
        "materialized='incremental', unique_key='order_id'",
        "duckdb",
        Some("merge"),
    )
    .unwrap();
    assert_eq!(
        (answer.strategy.as_str(), answer.strategy_source.as_str()),
        ("merge", "argument")
    );
}

// templates -------------------------------------------------------------

fn context() -> DbtContext {
    use cogitator_schemas::properties::{ColumnEntry, ColumnTest};
    let column = |name: &str, unique: bool| ColumnEntry {
        name: name.to_string(),
        tests: if unique {
            vec![ColumnTest {
                name: "unique".into(),
                strict: true,
            }]
        } else {
            Vec::new()
        },
        description: String::new(),
        name_pos: Default::default(),
        data_type: None,
    };
    let mut ctx = DbtContext::default();
    ctx.model_detail_map.insert(
        "orders".into(),
        crate::context::ModelDetails {
            columns: vec![
                column("order_id", true),
                column("order_date", false),
                column("updated_at", false),
            ],
            ..Default::default()
        },
    );
    ctx
}

fn actions(text: &str) -> Vec<CodeAction> {
    let mut docs = Documents::default();
    let uri = "file:///p/models/orders.sql".to_string();
    docs.open(uri.clone(), text, &cogitator_dialects::Dialect::default());
    incremental_template_actions(&context(), docs.get(&uri).unwrap(), &uri)
}

/// The text after an action's edits.
fn applied(text: &str, action: &CodeAction) -> String {
    let DocumentChange::Edit(edit) = &action.edit.as_ref().unwrap().document_changes[0] else {
        panic!("a text edit");
    };
    let table = LineTable::new(text);
    let offset = |p: &Position| {
        let start: usize = text
            .split_inclusive('\n')
            .take(p.line as usize)
            .map(str::len)
            .sum();
        let _ = &table;
        start + p.character as usize
    };
    let mut out = text.to_string();
    let mut edits = edit.edits.clone();
    edits.sort_by_key(|e| std::cmp::Reverse((e.range.start.line, e.range.start.character)));
    for e in edits {
        out.replace_range(offset(&e.range.start)..offset(&e.range.end), &e.new_text);
    }
    out
}

#[test]
fn a_table_is_offered_every_strategy_with_the_guessed_columns() {
    let text = "{{ config(materialized='table', tags=['daily']) }}\nselect * from {{ ref('stg_orders') }}\n";
    let titles: Vec<String> = actions(text).into_iter().map(|a| a.title).collect();
    assert_eq!(
        titles,
        vec![
            "Make incremental with the append strategy",
            "Make incremental with the merge strategy (unique_key='order_id')",
            "Make incremental with the delete+insert strategy (unique_key='order_id')",
            "Make incremental with the insert_overwrite strategy (partition_by={'field': 'order_date', 'data_type': 'date'})",
            "Make incremental with the microbatch strategy (event_time='updated_at', batch_size='day', begin='2024-01-01')",
        ]
    );
}

#[test]
fn the_merge_template_rewrites_the_config_and_adds_the_filter() {
    let text = "{{ config(materialized='table', tags=['daily']) }}\nselect * from {{ ref('stg_orders') }}\n";
    let merge = actions(text)
        .into_iter()
        .find(|a| a.title.contains("merge"))
        .unwrap();
    assert_eq!(
        applied(text, &merge),
        "{{ config(\n    materialized='incremental',\n    incremental_strategy='merge',\n    \
         unique_key='order_id',\n    tags=['daily']\n) }}\nselect * from {{ ref('stg_orders') }}\n\
         {% if is_incremental() %}\n    -- only the rows newer than the newest already built\n    \
         where updated_at > (select max(updated_at) from {{ this }})\n{% endif %}\n"
    );
}

#[test]
fn a_model_without_config_gets_one_and_an_and_after_its_where() {
    let text = "select * from {{ ref('stg_orders') }} where status = 'placed'";
    let append = &actions(text)[0];
    let after = applied(text, append);
    assert!(after.starts_with("{{ config(\n    materialized='incremental',\n    incremental_strategy='append'\n) }}\n\n"), "{after}");
    assert!(
        after.contains("    and updated_at > (select max(updated_at) from {{ this }})"),
        "{after}"
    );
}

#[test]
fn an_incremental_model_is_offered_the_other_strategies_and_no_filter() {
    let text = "{{ config(materialized='incremental', incremental_strategy='merge', unique_key='order_id') }}\nselect 1\n";
    let found = actions(text);
    let titles: Vec<&str> = found.iter().map(|a| a.title.as_str()).collect();
    assert!(
        titles.iter().all(|t| t.starts_with("Switch to the")),
        "{titles:?}"
    );
    assert!(
        !titles.iter().any(|t| t.contains("merge strategy")),
        "{titles:?}"
    );
    let append = found.iter().find(|a| a.title.contains("append")).unwrap();
    assert_eq!(
        applied(text, append),
        "{{ config(\n    materialized='incremental',\n    incremental_strategy='append'\n) }}\nselect 1\n"
    );
}
