//! `incr.could_be_incremental` over real SQL through the whole pipeline.
//! The fixture's `events` has a timestamp `ts`, so its growth is inferred
//! (a hint); the analysis itself has its case tables in
//! `canon::incrementality`.

mod filter_support;

use cogitator_ir::lir::Backend;
use cogitator_rules::{Evidence, Severity};
use filter_support::{no_findings, only, run, Case};

const RULE: &str = "incr.could_be_incremental";

fn table(body: &str) -> String {
    format!("{{{{ config(materialized='table') }}}}\n{body}")
}

#[test]
fn a_linear_table_over_a_growing_input_is_a_hint_with_append() {
    let sql = table("select id, ts, a + b as total from {{ ref('events') }} where a > 0");
    let r = run(Case::new(&sql, Backend::DuckDb, &[]));
    let f = only(&r, RULE);
    assert_eq!(f.severity, Severity::Hint, "inferred, not declared");
    assert_eq!(f.evidence, Evidence::Heuristic);
    assert!(
        f.message.contains("incremental_strategy='append'"),
        "{}",
        f.message
    );
    assert!(f.message.contains("`ts`"), "{}", f.message);
    assert!(
        f.message.contains("declaring `event_time: ts`"),
        "{}",
        f.message
    );
    // anchored on the config() call
    assert_eq!(f.span.start, 0);
}

#[test]
fn a_daily_rollup_replaces_its_newest_buckets() {
    let sql = table(
        "select date_trunc('day', ts) as day, count(*) as n \
         from {{ ref('events') }} group by 1",
    );
    let f = only(&run(Case::new(&sql, Backend::DuckDb, &[])), RULE).clone();
    assert!(f.message.contains("'delete+insert'"), "{}", f.message);
    assert!(f.message.contains("unique_key='day'"), "{}", f.message);
}

#[test]
fn views_incremental_models_and_whole_history_queries_are_silent() {
    let view = "{{ config(materialized='view') }}\nselect id, ts from {{ ref('events') }}";
    no_findings(&run(Case::new(view, Backend::DuckDb, &[])), RULE);
    let incremental =
        "{{ config(materialized='incremental') }}\nselect id, ts from {{ ref('events') }}";
    no_findings(&run(Case::new(incremental, Backend::DuckDb, &[])), RULE);
    let top = table("select id, ts from {{ ref('events') }} order by ts desc limit 10");
    no_findings(&run(Case::new(&top, Backend::DuckDb, &[])), RULE);
    let total = table("select count(*) as n from {{ ref('events') }}");
    no_findings(&run(Case::new(&total, Backend::DuckDb, &[])), RULE);
}

#[test]
fn jinja_the_plan_cannot_see_into_gives_no_verdict() {
    let sql = table("select id, ts from {{ ref('events') }} where {{ my_filter() }}");
    no_findings(&run(Case::new(&sql, Backend::DuckDb, &[])), RULE);
}
