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

fn with_text(sql: &str) -> Case<'_> {
    let mut case = Case::new(sql, Backend::DuckDb, &[]);
    case.with_text = true;
    case
}

/// The finding's one fix applied to `sql`.
fn rewritten(sql: &str) -> String {
    let r = run(with_text(sql));
    let f = only(&r, RULE);
    assert_eq!(f.fixes.len(), 1, "{f:?}");
    let fix = &f.fixes[0];
    assert_eq!(fix.safety, cogitator_rules::FixSafety::ChangesIntent);
    let mut out = sql.to_string();
    for edit in fix.edits.iter().rev() {
        out.replace_range(
            edit.span.start as usize..edit.span.end as usize,
            &edit.new_text,
        );
    }
    out
}

#[test]
fn the_rewrite_makes_an_appending_model_and_keeps_an_or_whole() {
    let sql = table("select id, ts from {{ ref('events') }} where a > 0 or b > 0");
    assert_eq!(
        rewritten(&sql),
        "{{ config(materialized='incremental', incremental_strategy='append') }}\n\
         select id, ts from {{ ref('events') }} where (a > 0 or b > 0)\n\
         {% if is_incremental() %}\n\
         and ((select max(ts) from {{ this }}) is null or ts > (select max(ts) from {{ this }}))\n\
         {% endif %}"
    );
}

#[test]
fn the_rewrite_adds_a_where_after_the_from_and_qualifies_by_alias() {
    let sql = table(
        "select e.id, e.ts from {{ ref('events') }} e join {{ ref('events') }} x on x.id = e.id",
    );
    // two reads of the driving input: no single place, no fix
    let r = run(with_text(&sql));
    assert!(r.findings(RULE).iter().all(|f| f.fixes.is_empty()));

    let sql = table("select date_trunc('day', e.ts) as day, count(*) as n from {{ ref('events') }} e group by 1");
    assert_eq!(
        rewritten(&sql),
        "{{ config(materialized='incremental', incremental_strategy='delete+insert', unique_key='day') }}\n\
         select date_trunc('day', e.ts) as day, count(*) as n from {{ ref('events') }} e\n\
         {% if is_incremental() %}\n\
         where ((select max(day) from {{ this }}) is null or e.ts >= (select max(day) from {{ this }}))\n\
         {% endif %} group by 1"
    );
}

#[test]
fn the_rewritten_model_is_an_incremental_model_with_its_guard() {
    let sql = table("select id, ts from {{ ref('events') }}");
    let after = rewritten(&sql);
    let r = run(Case::new(&after, Backend::DuckDb, &[]));
    no_findings(&r, RULE);
    no_findings(&r, "incr.missing_guard");
}

#[test]
fn the_rewrite_recomputes_the_groups_new_rows_touch() {
    let sql =
        table("select id, count(*) as n, max(ts) as last_ts from {{ ref('events') }} group by id");
    assert_eq!(
        rewritten(&sql),
        "{{ config(materialized='incremental', incremental_strategy='delete+insert', unique_key='id') }}\n\
         select id, count(*) as n, max(ts) as last_ts from {{ ref('events') }}\n\
         {% if is_incremental() %}\n\
         where ((select max(last_ts) from {{ this }}) is null or id in \
         (select id from {{ ref('events') }} where ts > (select max(last_ts) from {{ this }})))\n\
         {% endif %} group by id"
    );
    // without a column holding each group's newest ts there is no rewrite
    let blind = table("select id, count(*) as n from {{ ref('events') }} group by id");
    let r = run(with_text(&blind));
    assert!(only(&r, RULE).fixes.is_empty());
}
