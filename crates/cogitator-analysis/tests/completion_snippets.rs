//! Model-aware snippets over `testdata/type_check`: the `is_incremental()`
//! block filters on the model's real growing column, and the incremental
//! `config()` carries its real merge key.

use std::path::PathBuf;

use cogitator_analysis::{build_context, features, Documents};
use cogitator_lsp::{adapt, insert_text_format, CompletionItem, CompletionSupport, Position};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/type_check")
}

fn complete(model: &str, text: &str, line: u32, character: u32) -> Vec<CompletionItem> {
    let ctx = build_context(root().to_str().unwrap());
    let uri = cogitator_lsp::fs_path_to_uri(&root().join(format!("models/{model}.sql")));
    let mut docs = Documents::default();
    docs.open(uri.clone(), text, &ctx.dialect);
    features::completion_with(
        &ctx,
        docs.get(&uri).unwrap(),
        Position { line, character },
        &features::ModelHints::default(),
    )
}

fn snippet<'a>(items: &'a [CompletionItem], label: &str) -> &'a CompletionItem {
    items
        .iter()
        .find(|i| i.label == label)
        .unwrap_or_else(|| panic!("{label}: {items:#?}"))
}

#[test]
fn the_block_filters_on_the_declared_event_time() {
    let text = "select order_id, ordered_on from {{ ref('stg_orders') }}\nis_inc";
    let items = complete("late_events", text, 1, 6);
    let block = snippet(&items, "is_incremental");
    assert_eq!(block.insert_text_format, Some(insert_text_format::SNIPPET));
    assert_eq!(
        block.insert_text,
        "{% if is_incremental() %}\n    where ordered_on > (select max(ordered_on) from {{ this }})\n{% endif %}$0"
    );
}

#[test]
fn a_guessed_column_is_a_tab_stop_and_and_follows_a_where() {
    let text = "select event_id, created_at from {{ source('shop', 'orders') }}\nwhere event_id > 0\nis_incr";
    let items = complete("events_feed", text, 2, 7);
    let block = snippet(&items, "is_incremental");
    assert_eq!(
        block.insert_text,
        "{% if is_incremental() %}\n    and ${1:created_at} > (select max(${1:created_at}) from {{ this }})\n{% endif %}$0"
    );
    // a client without snippets gets the guess as plain text
    let plain = adapt(block.clone(), CompletionSupport::default());
    assert!(
        plain
            .insert_text
            .contains("and created_at > (select max(created_at)"),
        "{}",
        plain.insert_text
    );
    assert_eq!(plain.insert_text_format, None);
}

#[test]
fn inside_an_if_tag_only_the_condition_completes() {
    let items = complete("late_events", "select 1\n{% if is_", 1, 10);
    assert_eq!(items.len(), 1, "{items:#?}");
    assert!(items[0]
        .insert_text
        .starts_with("is_incremental() %}\n    where ordered_on >"));
    let closed = complete("late_events", "select 1\n{% if  %}", 1, 6);
    assert_eq!(closed[0].insert_text, "is_incremental()$0");
}

#[test]
fn config_takes_the_primary_key_or_the_combination() {
    let pk = complete("late_events", "conf", 0, 4);
    assert_eq!(
        snippet(&pk, "config incremental").insert_text,
        "{{ config(materialized='incremental', unique_key='order_id') }}$0"
    );
    let combo = complete("events_feed", "conf", 0, 4);
    assert_eq!(
        snippet(&combo, "config incremental").insert_text,
        "{{ config(materialized='incremental', unique_key=['created_at', 'event_id']) }}$0"
    );
    let unknown = complete("channel_totals", "conf", 0, 4);
    assert!(snippet(&unknown, "config incremental")
        .insert_text
        .contains("unique_key='${1:id}'"));
}

#[test]
fn no_snippets_outside_a_model_or_for_short_words() {
    assert!(complete("late_events", "co", 0, 2)
        .iter()
        .all(|i| i.label != "config incremental"));
    let ctx = build_context(root().to_str().unwrap());
    let mut docs = Documents::default();
    docs.open(
        "file:///tmp/scratch.sql".to_string(),
        "is_inc",
        &ctx.dialect,
    );
    let items = features::completion(
        &ctx,
        docs.get("file:///tmp/scratch.sql").unwrap(),
        Position {
            line: 0,
            character: 6,
        },
    );
    assert!(items.iter().all(|i| i.label != "is_incremental"));
}
