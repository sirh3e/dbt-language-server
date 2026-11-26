//! `join.fanout` through the real pipeline: when a join may duplicate
//! rows, what proves it cannot, and the deduplicating fix.

mod join_support;

use std::sync::Arc;

use cogitator_ir::lir::Backend;
use cogitator_rules::{Finding, RelationName, Severity, Subject};
use join_support::{all_needs, apply, findings, model, run, schemas};

const RULE: &str = "join.fanout";

fn fanout(sql: &str) -> Vec<Finding> {
    findings(sql, RULE)
}

fn with_unique(name: &str, column: &str) -> cogitator_ir::schema::ProjectSchemas {
    let mut out = schemas();
    let cols = ["id", "k", "state", "flag"].map(|c| (c, c == column));
    out.models.insert(Arc::from(name), model(&cols));
    out
}

fn only(mut found: Vec<Finding>) -> Finding {
    assert_eq!(found.len(), 1, "{found:#?}");
    found.remove(0)
}

#[test]
fn a_join_on_a_key_not_known_unique_is_a_hint_naming_both_sides() {
    let sql =
        "select x.id, y.state from {{ ref('a') }} as x join {{ ref('b') }} as y on x.id = y.k";
    let found = only(fanout(sql));
    assert_eq!(found.severity, Severity::Hint);
    assert_eq!(
        found.message,
        "this join may duplicate rows of a: k is not known to be unique in b"
    );
    assert_eq!(
        found.subject,
        Some(Subject::UniqueKey {
            relation: RelationName::Model("b".into()),
            column: "k".into(),
        })
    );
}

#[test]
fn a_unique_test_on_the_joined_key_silences_it() {
    let sql =
        "select x.id, y.state from {{ ref('a') }} as x left join {{ ref('b') }} as y on x.id = y.k";
    assert!(run(sql, with_unique("b", "k"), all_needs(), RULE).is_empty());
    // the driving side's uniqueness is not what matters
    assert_eq!(run(sql, with_unique("a", "id"), all_needs(), RULE).len(), 1);
}

#[test]
fn a_composite_key_covered_by_the_join_silences_it() {
    let mut schemas = schemas();
    let b = model(&[
        ("id", false),
        ("k", false),
        ("state", false),
        ("flag", false),
    ]);
    let mut b = (*b).clone();
    b.unique_sets = vec![vec![Arc::from("k"), Arc::from("state")]];
    schemas.models.insert(Arc::from("b"), Arc::new(b));
    let both = "select x.id from {{ ref('a') }} as x join {{ ref('b') }} as y \
                on x.id = y.k and x.state = y.state";
    assert!(run(both, schemas.clone(), all_needs(), RULE).is_empty());
    let one = "select x.id from {{ ref('a') }} as x join {{ ref('b') }} as y on x.id = y.k";
    assert_eq!(run(one, schemas, all_needs(), RULE).len(), 1);
}

#[test]
fn a_group_by_on_the_key_restores_the_grain() {
    let sql = "select x.id, count(*) as n from {{ ref('a') }} as x \
               join {{ ref('b') }} as y on x.id = y.k group by x.id";
    assert!(fanout(sql).is_empty());
}

#[test]
fn an_aggregated_or_deduplicated_joined_side_is_unique() {
    let grouped = "select x.id, s.n from {{ ref('a') }} as x left join \
                   (select k, count(*) as n from {{ ref('b') }} group by k) as s on x.id = s.k";
    assert!(fanout(grouped).is_empty());
    let first = "with b1 as (select * from {{ ref('b') }} \
                 qualify row_number() over (partition by k order by k) = 1) \
                 select x.id from {{ ref('a') }} as x join b1 on x.id = b1.k";
    assert!(fanout(first).is_empty(), "{:#?}", fanout(first));
}

#[test]
fn distinct_over_the_join_is_left_to_distinct_fanout() {
    let sql =
        "select distinct x.id from {{ ref('a') }} as x join {{ ref('b') }} as y on x.id = y.k";
    assert!(fanout(sql).is_empty());
}

#[test]
fn the_dedup_fix_adds_a_cte_and_reads_it() {
    let sql =
        "select x.id, y.state from {{ ref('a') }} as x join {{ ref('b') }} as y on x.id = y.k";
    let found = only(fanout(sql));
    let fix = found.fixes.first().expect("dedup fix");
    assert_eq!(fix.title, "Deduplicate b on k");
    let fixed = apply(sql, fix);
    assert_eq!(
        fixed,
        "with b_dedup as (\n    select * from (\n        select *, row_number() over (partition by k order by k) as dedup_row\n        from {{ ref('b') }}\n    ) as deduplicated\n    where dedup_row = 1\n)\nselect x.id, y.state from {{ ref('a') }} as x join b_dedup as y on x.id = y.k"
    );
    // the fixed text no longer fans out
    assert!(fanout(&fixed).is_empty(), "{:#?}", fanout(&fixed));
}

#[test]
fn qualify_dialects_get_a_qualify_cte_after_existing_ones() {
    let sql = "with a1 as (select * from {{ ref('a') }})\nselect a1.id from a1 join {{ ref('b') }} as y on a1.id = y.k";
    let mut out = join_support::run_raw_with(
        sql,
        schemas(),
        all_needs(),
        join_support::Extras {
            dialect: Some(Backend::DuckDb),
            ..Default::default()
        },
    )
    .findings;
    out.retain(|f| f.id.as_str() == RULE);
    let fixed = apply(sql, &only(out).fixes[0]);
    assert!(
        fixed.starts_with("with b_dedup as (\n    select * from {{ ref('b') }}\n    qualify row_number() over (partition by k order by k) = 1\n),\na1 as"),
        "{fixed}"
    );
}
