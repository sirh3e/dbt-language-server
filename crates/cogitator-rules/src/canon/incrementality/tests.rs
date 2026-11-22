//! The classification over real SQL: the front end and lowering build the
//! MIR of each case, and the verdict is compared with the DBSP class the
//! module documentation gives its operators.

use std::collections::BTreeMap;
use std::sync::Arc;

use cogitator_dialects::Dialect;
use cogitator_ir::common::ModelConfig;
use cogitator_ir::origin::{SourceOrigin, TemplateVariant};
use cogitator_ir::schema::{Completeness, ModelSchema, ProjectSchemas, SchemaCol};
use cogitator_ir::types::{ColType, Nullability, SqlType, Tz};
use cogitator_lower::lower_to_mir;
use cogitator_sql_front::{build_hir, HirInput, SchemaEnv};

use super::*;

fn col(name: &str, ty: SqlType) -> SchemaCol {
    SchemaCol {
        name: Arc::from(name),
        ty: ColType::new(ty, Nullability::Unknown),
        unique: false,
    }
}

fn model(cols: Vec<SchemaCol>) -> Arc<ModelSchema> {
    Arc::new(ModelSchema {
        columns: Arc::from(cols),
        completeness: Completeness::Closed,
        fingerprint: 0,
    })
}

/// events: an append-only log with a heuristic event column `ts`;
/// users: a dimension; payments: a second log; snap: a snapshot.
fn env() -> SchemaEnv {
    let int = SqlType::Int { bits: Some(64) };
    let ts = SqlType::Timestamp { tz: Tz::Unknown };
    let mut schemas = ProjectSchemas::default();
    let mut add = |name: &str, cols| {
        schemas.models.insert(Arc::from(name), model(cols));
    };
    add(
        "events",
        vec![
            col("id", int.clone()),
            col("user_id", int.clone()),
            col("ts", ts.clone()),
            col(
                "amount",
                SqlType::Decimal {
                    p: Some(18),
                    s: Some(2),
                },
            ),
            col("price", SqlType::Float { bits: Some(64) }),
            col("status", SqlType::Text),
        ],
    );
    add(
        "users",
        vec![col("id", int.clone()), col("name", SqlType::Text)],
    );
    add(
        "payments",
        vec![
            col("id", int.clone()),
            col("event_id", int.clone()),
            col("paid_at", ts.clone()),
        ],
    );
    add("snap", vec![col("id", int.clone()), col("valid_from", ts)]);
    add("logs", vec![col("id", int), col("seq", SqlType::Text)]);
    SchemaEnv {
        schemas: Arc::new(schemas),
        model: Some(Arc::from("probe")),
    }
}

fn analysis_with(sql: &str, inputs: Option<&IncrementalInputs>) -> Analysis {
    let dialect = Dialect::new("duckdb");
    let input = HirInput {
        text: sql,
        dialect: &dialect,
        origin: SourceOrigin::Template {
            variant: TemplateVariant::Plain,
        },
        config: ModelConfig::default(),
    };
    let hir = build_hir(&input, &env()).value.expect("a HIR");
    let mir = lower_to_mir(&hir).value.expect("a MIR");
    analyse(&mir, inputs)
}

fn analysis(sql: &str) -> Analysis {
    analysis_with(sql, None)
}

fn blockers(a: &Analysis) -> Vec<Blocker> {
    match &a.verdict {
        Verdict::Keep(b) => b.iter().map(|b| b.blocker.clone()).collect(),
        other => panic!("not kept: {other:?}"),
    }
}

#[test]
fn linear_queries_over_a_growing_input_are_appended() {
    for sql in [
        "select id, ts from {{ ref('events') }}",
        "select id, ts, amount * 2 as doubled from {{ ref('events') }} where status = 'ok'",
        "select e.id, e.ts, u.name from {{ ref('events') }} e join {{ ref('users') }} u on u.id = e.user_id",
        "select e.id, e.ts, u.name from {{ ref('events') }} e left join {{ ref('users') }} u on u.id = e.user_id",
        "select id, ts from {{ ref('events') }} union all select id, paid_at from {{ ref('payments') }}",
        "with recent as (select id, ts from {{ ref('events') }}) select id, ts from recent",
        "select id, date_trunc('day', ts) as day from {{ ref('events') }}",
    ] {
        let a = analysis(sql);
        assert!(
            matches!(&a.verdict, Verdict::Append { .. }),
            "{sql}\n{:?}",
            a.verdict
        );
    }
    let a = analysis("select e.id, e.ts, u.name from {{ ref('events') }} e join {{ ref('users') }} u on u.id = e.user_id");
    let Verdict::Append { inputs, out_column } = &a.verdict else {
        unreachable!()
    };
    assert_eq!(out_column, "ts");
    assert_eq!(inputs[0].relation, "events");
    assert!(!inputs[0].declared, "inferred from the column's name");
    assert!(a.notes.contains(&Note::AssumesStatic("users".into())));
}

#[test]
fn a_group_by_a_bucket_of_the_event_time_replaces_buckets() {
    let a = analysis(
        "select date_trunc('day', ts) as day, status, count(*) as n, sum(amount) as total \
         from {{ ref('events') }} group by 1, 2",
    );
    let Verdict::ByBucket {
        input,
        bucket,
        keys,
    } = &a.verdict
    else {
        panic!("{:?}", a.verdict)
    };
    assert_eq!(
        (input.relation.as_str(), input.column.as_str()),
        ("events", "ts")
    );
    assert_eq!(bucket, "day");
    assert_eq!(keys, &["day".to_string(), "status".to_string()]);
}

#[test]
fn a_group_by_keys_recomputes_touched_groups() {
    let a = analysis(
        "select user_id, count(*) as n, max(ts) as last_seen, sum(price) as spend \
         from {{ ref('events') }} group by user_id",
    );
    let Verdict::ByKey {
        keys,
        strategy,
        high_water,
        ..
    } = &a.verdict
    else {
        panic!("{:?}", a.verdict)
    };
    assert_eq!(keys, &["user_id".to_string()]);
    assert_eq!(*strategy, Strategy::DeleteInsert);
    assert_eq!(high_water.as_deref(), Some("last_seen"));
    assert!(a.notes.contains(&Note::FloatSum), "{:?}", a.notes);
    assert!(a.notes.contains(&Note::MinMax));

    let distinct = analysis(
        "select user_id, count(distinct status) as kinds from {{ ref('events') }} group by user_id",
    );
    assert!(matches!(
        distinct.verdict,
        Verdict::ByKey {
            high_water: None,
            ..
        }
    ));
    assert!(distinct.notes.contains(&Note::Holistic("count".into())));

    let dedupe = analysis("select distinct user_id, status from {{ ref('events') }}");
    assert!(
        matches!(
            dedupe.verdict,
            Verdict::ByKey {
                strategy: Strategy::Merge,
                ..
            }
        ),
        "{:?}",
        dedupe.verdict
    );

    let window = analysis(
        "select id, user_id, row_number() over (partition by user_id order by ts) as n \
         from {{ ref('events') }}",
    );
    assert!(
        matches!(&window.verdict, Verdict::ByKey { keys, .. } if keys == &["user_id".to_string()]),
        "{:?}",
        window.verdict
    );
}

#[test]
fn whole_history_operators_keep_the_table() {
    let cases: &[(&str, Blocker)] = &[
        ("select id, ts from {{ ref('events') }} order by ts desc limit 10", Blocker::Limit),
        (
            "select id, ts, row_number() over (order by ts) as n from {{ ref('events') }}",
            Blocker::WindowOverWholeTable,
        ),
        ("select count(*) as n from {{ ref('events') }}", Blocker::AggregateOverWholeTable),
        (
            "select id, ts from {{ ref('events') }} except select id, paid_at from {{ ref('payments') }}",
            Blocker::SetDifference,
        ),
        (
            "select id, ts, current_timestamp as loaded from {{ ref('events') }}",
            Blocker::NonDeterministic("current_timestamp".into()),
        ),
        (
            "select id, ts from {{ ref('events') }} where ts > now() - interval '7 days'",
            Blocker::TemporalFilter("now".into()),
        ),
        (
            "select e.id, e.ts from {{ ref('events') }} e full join {{ ref('payments') }} p on p.event_id = e.id",
            Blocker::FullJoin,
        ),
        (
            "select u.id, e.ts from {{ ref('users') }} u left join {{ ref('events') }} e on e.user_id = u.id",
            Blocker::OuterJoinOnGrowingSide,
        ),
        (
            "select id, ts from {{ ref('events') }} where user_id in (select event_id from {{ ref('payments') }})",
            Blocker::SubqueryOverGrowingInput,
        ),
        (
            "select user_id, n from (select user_id, count(*) as n from {{ ref('events') }} group by user_id) g \
             order by n desc limit 5",
            Blocker::Limit,
        ),
    ];
    for (sql, want) in cases {
        let a = analysis(sql);
        assert!(blockers(&a).contains(want), "{sql}\n{:?}", a.verdict);
    }
}

#[test]
fn inputs_that_never_grow_have_nothing_to_increment() {
    assert_eq!(
        analysis("select id, name from {{ ref('users') }}").verdict,
        Verdict::Static
    );
}

#[test]
fn a_growing_input_without_a_readable_event_column_stays_a_table() {
    let logs = IncrementalInputs {
        by_name: BTreeMap::from([(
            Arc::from("logs"),
            InputFacts {
                growth: InputGrowth::Append,
                event_time: None,
                inferred: false,
            },
        )]),
    };
    let a = analysis_with("select id, seq from {{ ref('logs') }}", Some(&logs));
    assert_eq!(blockers(&a), vec![Blocker::NoMonotoneColumn]);
}

#[test]
fn declarations_win_over_names_and_a_snapshot_changes() {
    let declared = IncrementalInputs {
        by_name: BTreeMap::from([
            (
                Arc::from("payments"),
                InputFacts {
                    growth: InputGrowth::Append,
                    event_time: Some(Arc::from("paid_at")),
                    inferred: false,
                },
            ),
            (
                Arc::from("snap"),
                InputFacts {
                    growth: InputGrowth::Changing,
                    event_time: None,
                    inferred: false,
                },
            ),
        ]),
    };
    let a = analysis_with(
        "select id, paid_at from {{ ref('payments') }}",
        Some(&declared),
    );
    let Verdict::Append { inputs, out_column } = &a.verdict else {
        panic!("{:?}", a.verdict)
    };
    assert!(inputs[0].declared);
    assert_eq!(out_column, "paid_at");

    let snap = analysis_with(
        "select id, valid_from from {{ ref('snap') }}",
        Some(&declared),
    );
    assert_eq!(blockers(&snap), vec![Blocker::ChangingInput("snap".into())]);
}

#[test]
fn two_growing_join_inputs_are_bilinear() {
    let a = analysis(
        "select e.id, e.ts, p.paid_at from {{ ref('events') }} e join {{ ref('payments') }} p on p.event_id = e.id",
    );
    assert!(
        matches!(a.verdict, Verdict::Append { .. }),
        "{:?}",
        a.verdict
    );
    assert!(
        a.notes.contains(&Note::Bilinear {
            other: "payments".into()
        }),
        "{:?}",
        a.notes
    );
}

#[test]
fn opaque_sql_gets_no_verdict() {
    let a = analysis("select id, ts from {{ ref('events') }} where {{ my_filter() }}");
    assert_eq!(a.verdict, Verdict::Unknown, "{:?}", a.verdict);
}

mod sexps {
    use super::*;
    use crate::canon::incrementality::annotate::analysis_sexp;
    use crate::canon::incrementality::frontier::{frontier, frontier_sexp};

    fn mir_of(sql: &str) -> cogitator_ir::mir::Mir {
        let dialect = Dialect::new("duckdb");
        let input = HirInput {
            text: sql,
            dialect: &dialect,
            origin: SourceOrigin::Template {
                variant: TemplateVariant::Plain,
            },
            config: ModelConfig::default(),
        };
        let hir = build_hir(&input, &env()).value.expect("a HIR");
        lower_to_mir(&hir).value.expect("a MIR")
    }

    const RANKED: &str = "with by_user as (\n  select user_id, sum(amount) as total, max(ts) as last_ts\n  from {{ ref('events') }}\n  where status = 'ok'\n  group by user_id\n)\nselect user_id, total, rank() over (order by total desc) as rnk\nfrom by_user";

    #[test]
    fn the_analysis_annotates_every_node_with_its_class() {
        let mir = mir_of(RANKED);
        let a = analyse(&mir, None);
        insta::assert_snapshot!(analysis_sexp(&mir, mir.root, "orders_ranked", &a).pretty(100));
    }

    #[test]
    fn a_reducing_cte_under_a_blocker_is_a_cut() {
        let mir = mir_of(RANKED);
        let f = frontier(&mir, None);
        assert_eq!(f.cuts.len(), 1, "{f:?}");
        assert_eq!(f.cuts[0].cte, "by_user");
        assert_eq!(f.cuts[0].reduces, "aggregate");
        assert!(matches!(f.cuts[0].analysis.verdict, Verdict::ByKey { .. }));
        insta::assert_snapshot!(frontier_sexp(&mir, "orders_ranked", &f).pretty(100));
    }

    #[test]
    fn parts_that_save_nothing_or_cannot_stand_alone_are_refused() {
        use crate::canon::incrementality::frontier::Refusal;
        // a global window straight over the raw scan: nothing to cut
        let raw = mir_of("select id, ts, rank() over (order by ts) as r from {{ ref('events') }}");
        assert_eq!(
            frontier(&raw, None).refusals,
            vec![Refusal::NoReduction { cte: None }]
        );
        // the aggregate and the blocker in one SELECT
        let one = mir_of(
            "select user_id, sum(amount) as total, rank() over (order by sum(amount)) as r \
             from {{ ref('events') }} group by user_id",
        );
        assert_eq!(frontier(&one, None).refusals, vec![Refusal::NoBoundary]);
        // a CTE that only projects
        let thin = mir_of(
            "with e as (select id, ts from {{ ref('events') }}) \
             select id, ts, rank() over (order by ts) as r from e",
        );
        assert_eq!(
            frontier(&thin, None).refusals,
            vec![Refusal::NoReduction {
                cte: Some("e".into())
            }]
        );
        // an incremental model has no frontier
        let fine = mir_of("select id, ts from {{ ref('events') }}");
        assert_eq!(frontier(&fine, None), Default::default());
    }
}
