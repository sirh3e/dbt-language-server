use std::collections::HashMap;

use super::*;

fn inputs() -> PreviewInputs {
    PreviewInputs {
        macro_sources: Vec::new(),
        model_relations: HashMap::from([(
            "stg_events".to_string(),
            "jaffle.main.stg_events".to_string(),
        )]),
        source_relations: HashMap::new(),
        vars: HashMap::new(),
        target_name: "dev".to_string(),
        is_incremental: false,
        this: Some("jaffle.main.events".to_string()),
    }
}

const EVENTS: &str = "{{ config(materialized='incremental') }}\n\
select id, ts\n\
from {{ ref('stg_events') }}\n\
{% if is_incremental() %}\n\
where ts > (select max(ts) from {{ this }})\n\
{% endif %}\n";

#[test]
fn the_two_renders_differ_by_the_guarded_filter() {
    let diff = incremental_diff("events", EVENTS, &mut inputs()).expect("renders");
    assert!(!diff.identical);
    assert!(
        !diff.full_refresh.contains("where ts"),
        "{}",
        diff.full_refresh
    );
    assert!(
        diff.incremental
            .contains("where ts > (select max(ts) from jaffle.main.events)"),
        "{}",
        diff.incremental
    );
    assert!(
        diff.diff
            .starts_with("--- events (full refresh)\n+++ events (incremental)\n"),
        "{}",
        diff.diff
    );
    assert!(
        diff.diff
            .lines()
            .any(|l| l == "+where ts > (select max(ts) from jaffle.main.events)"),
        "{}",
        diff.diff
    );
    assert!(diff
        .diff
        .lines()
        .all(|l| !l.starts_with('-') || l.starts_with("---")));
}

#[test]
fn a_model_without_a_guard_renders_identically() {
    let body = "{{ config(materialized='incremental') }}\nselect 1 as x\n";
    let diff = incremental_diff("m", body, &mut inputs()).expect("renders");
    assert!(diff.identical);
    assert_eq!(diff.diff, "");
    assert_eq!(diff.full_refresh, diff.incremental);
}

#[test]
fn render_errors_are_guidance() {
    let err = incremental_diff("m", "select {{ var('nope') }}", &mut inputs()).unwrap_err();
    assert!(err.to_message().contains("nope"), "{}", err.to_message());
}

#[test]
fn materialized_reads_the_last_literal_config() {
    for (text, want) in [
        ("{{ config(materialized='incremental') }}", Some("incremental")),
        ("{{ config(materialized=\"table\", unique_key=['a']) }}", Some("table")),
        (
            "{{ config(tags=['x'], materialized='view') }}\n{{ config(materialized='incremental') }}",
            Some("incremental"),
        ),
        // a nested dict key named materialized is not the config's
        ("{{ config(meta={'materialized': 'table'}) }}", None),
        ("{{ config(materialized=var('m')) }}", None),
        ("select 1", None),
        ("{% if x %}{{ config(materialized='incremental') }}{% endif %}", Some("incremental")),
        ("{{ broken(", None),
    ] {
        assert_eq!(config_materialized(text).as_deref(), want, "{text}");
    }
}

#[test]
fn the_file_wins_over_the_manifest() {
    for (text, manifest, want) in [
        ("select 1", Some("incremental"), true),
        ("select 1", Some("table"), false),
        ("select 1", None, false),
        (
            "{{ config(materialized='table') }}",
            Some("incremental"),
            false,
        ),
        (
            "{{ config(materialized='incremental') }}",
            Some("table"),
            true,
        ),
    ] {
        assert_eq!(
            is_incremental_model(text, manifest),
            want,
            "{text} {manifest:?}"
        );
    }
}

#[test]
fn the_lens_only_marks_incremental_models() {
    let mut ctx = DbtContext::default();
    ctx.model_detail_map
        .insert("events".to_string(), Default::default());
    let uri = "file:///p/models/events.sql";
    let lens = incremental_lens(&ctx, uri, EVENTS, None).expect("a lens");
    assert_eq!(lens.command.title, INCREMENTAL_LENS_TITLE);
    assert_eq!(lens.command.command, "dbt.incrementalDiff");
    assert_eq!(lens.command.arguments[0]["show"], true);
    assert!(incremental_lens(&ctx, uri, "select 1", None).is_none());
    // an unknown model gets none
    assert!(incremental_lens(&ctx, "file:///p/models/other.sql", EVENTS, None).is_none());
}
