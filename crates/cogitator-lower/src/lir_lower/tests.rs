//! MIR→LIR over hand-built plans.

use cogitator_ir::common::{BinaryOp, HoleKind, JoinKind, Materialization, SetOpKind};
use cogitator_ir::lir::{
    print_lir, Backend, CtePolicy, Estimate, JoinHint, KeyKind, Lir, PhysColumn, PhysOp,
    PhysicalFacts, PruneUse, TableFacts,
};
use cogitator_ir::mir::{ColId, Mir, RelOp, ScalarId, ScalarKind, TableId};
use cogitator_ir::testkit::{int_col, MirBuilder};
use cogitator_ir::{ColType, Fidelity, Nullability, SqlType};

use crate::backends::for_backend;
use crate::lower_to_lir;

fn ts_col() -> ColType {
    ColType::new(
        SqlType::Timestamp {
            tz: cogitator_ir::types::Tz::Unknown,
        },
        Nullability::Unknown,
    )
}

fn backend(b: Backend) -> &'static dyn crate::DialectBackend {
    for_backend(b).expect("registered")
}

fn lower(mir: &Mir, b: Backend, phys: &PhysicalFacts) -> Lir {
    let lowered = lower_to_lir(mir, backend(b), phys);
    lowered.assert_clean();
    lowered.value.expect("a plan")
}

/// `t(ts partition, a cluster, b cluster, c)` scanned as table 0.
struct Fixture {
    b: MirBuilder,
    scan: cogitator_ir::mir::RelId,
    cols: Vec<ColId>,
    phys: PhysicalFacts,
}

fn fixture() -> Fixture {
    let mut b = MirBuilder::new();
    let scan = b.scan(
        "t",
        &[
            ("ts", ts_col()),
            ("a", int_col()),
            ("b", int_col()),
            ("c", int_col()),
        ],
    );
    let cols = b.out(scan);
    let columns = [
        ("ts", KeyKind::Partition),
        ("a", KeyKind::Cluster),
        ("b", KeyKind::Cluster),
    ]
    .iter()
    .map(|(name, key)| PhysColumn {
        name: b.sym(name),
        ty: SqlType::Unknown,
        key: Some(*key),
    })
    .collect();
    let mut phys = PhysicalFacts::default();
    phys.tables.insert(
        0,
        TableFacts {
            columns,
            row_count: Some(1000),
        },
    );
    Fixture {
        b,
        scan,
        cols,
        phys,
    }
}

impl Fixture {
    fn cmp(&mut self, col: ColId, op: BinaryOp) -> ScalarId {
        let (l, r) = (self.b.col_ref(col), self.b.number("1"));
        self.b.binary(op, l, r)
    }

    fn wrapped(&mut self, col: ColId, func: &str) -> ScalarId {
        let inner = self.b.col_ref(col);
        let call = self.b.call(func, vec![inner]);
        let r = self.b.number("1");
        self.b.binary(BinaryOp::Eq, call, r)
    }

    fn filtered(mut self, conjuncts: Vec<ScalarId>) -> (Mir, PhysicalFacts) {
        let root = self.b.filter(self.scan, conjuncts);
        (self.b.finish(root), self.phys)
    }

    fn scanned(self) -> (Mir, PhysicalFacts) {
        let scan = self.scan;
        (self.b.finish(scan), self.phys)
    }
}

fn usages(lir: &Lir) -> Vec<(KeyKind, PruneUse)> {
    lir.nodes
        .values()
        .find_map(|n| match &n.op {
            PhysOp::TableScan { pruning, .. } => {
                Some(pruning.iter().map(|k| (k.kind, k.usage)).collect())
            }
            _ => None,
        })
        .unwrap_or_default()
}

fn usage(lir: &Lir, index: usize) -> PruneUse {
    usages(lir)[index].1
}

#[test]
fn a_bare_filter_uses_the_partition_key() {
    let mut f = fixture();
    let ts = f.cols[0];
    let pred = f.cmp(ts, BinaryOp::Eq);
    let (mir, phys) = f.filtered(vec![pred]);
    let lir = lower(&mir, Backend::BigQuery, &phys);
    assert_eq!(usage(&lir, 0), PruneUse::Used(pred));
    insta::assert_snapshot!(print_lir(&lir, &mir));
}

#[test]
fn an_unfiltered_key_is_not_filtered() {
    let (mir, phys) = fixture().scanned();
    let lir = lower(&mir, Backend::BigQuery, &phys);
    assert_eq!(usage(&lir, 0), PruneUse::NotFiltered);
}

#[test]
fn a_filter_above_a_project_is_unknown_not_not_filtered() {
    let mut f = fixture();
    let ts = f.cols[0];
    let pred = f.cmp(ts, BinaryOp::Eq);
    let r = f.b.col_ref(ts);
    let project = f.b.project(f.scan, vec![(ts, r)]);
    let root = f.b.filter(project, vec![pred]);
    let mir = f.b.finish(root);
    let lir = lower(&mir, Backend::BigQuery, &f.phys);
    assert_eq!(usage(&lir, 0), PruneUse::Unknown);
}

#[test]
fn bigquery_documented_function_on_partition_is_unknown() {
    let mut f = fixture();
    let ts = f.cols[0];
    let pred = f.wrapped(ts, "TIMESTAMP_TRUNC");
    let (mir, phys) = f.filtered(vec![pred]);
    assert_eq!(
        usage(&lower(&mir, Backend::BigQuery, &phys), 0),
        PruneUse::Unknown
    );
}

#[test]
fn bigquery_undocumented_function_on_partition_blocks() {
    let mut f = fixture();
    let ts = f.cols[0];
    let pred = f.wrapped(ts, "format_date");
    let (mir, phys) = f.filtered(vec![pred]);
    assert!(matches!(
        usage(&lower(&mir, Backend::BigQuery, &phys), 0),
        PruneUse::Blocked { .. }
    ));
}

#[test]
fn bigquery_arithmetic_on_the_partition_column_blocks() {
    let mut f = fixture();
    let ts = f.cols[0];
    let col = f.b.col_ref(ts);
    let one = f.b.number("1");
    let plus = f.b.binary(BinaryOp::Plus, col, one);
    let two = f.b.number("2");
    let pred = f.b.binary(BinaryOp::Gt, plus, two);
    let (mir, phys) = f.filtered(vec![pred]);
    assert!(matches!(
        usage(&lower(&mir, Backend::BigQuery, &phys), 0),
        PruneUse::Blocked { .. }
    ));
}

#[test]
fn bigquery_comparison_to_another_column_blocks() {
    let mut f = fixture();
    let (a, c) = (f.b.col_ref(f.cols[1]), f.b.col_ref(f.cols[3]));
    let pred = f.b.binary(BinaryOp::Eq, a, c);
    let (mir, phys) = f.filtered(vec![pred]);
    assert!(matches!(
        usage(&lower(&mir, Backend::BigQuery, &phys), 1),
        PruneUse::Blocked { .. }
    ));
}

#[test]
fn bigquery_filter_skipping_the_first_cluster_column_is_blocked() {
    let mut f = fixture();
    let b_col = f.cols[2];
    let pred = f.cmp(b_col, BinaryOp::Eq);
    let (mir, phys) = f.filtered(vec![pred]);
    let lir = lower(&mir, Backend::BigQuery, &phys);
    assert_eq!(usage(&lir, 1), PruneUse::NotFiltered);
    assert!(matches!(usage(&lir, 2), PruneUse::Blocked { by, .. } if by == pred));
}

#[test]
fn bigquery_filters_in_cluster_order_are_used() {
    let mut f = fixture();
    let (a, b) = (f.cols[1], f.cols[2]);
    let (p1, p2) = (f.cmp(a, BinaryOp::Eq), f.cmp(b, BinaryOp::Gt));
    let (mir, phys) = f.filtered(vec![p2, p1]);
    let lir = lower(&mir, Backend::BigQuery, &phys);
    assert_eq!(usage(&lir, 1), PruneUse::Used(p1));
    assert_eq!(usage(&lir, 2), PruneUse::Used(p2));
}

#[test]
fn bigquery_or_across_columns_blocks_the_partition() {
    let mut f = fixture();
    let (ts, c) = (f.cols[0], f.cols[3]);
    let (l, r) = (f.cmp(ts, BinaryOp::Eq), f.cmp(c, BinaryOp::Eq));
    let pred = f.b.binary(BinaryOp::Or, l, r);
    let (mir, phys) = f.filtered(vec![pred]);
    assert!(matches!(
        usage(&lower(&mir, Backend::BigQuery, &phys), 0),
        PruneUse::Blocked { .. }
    ));
}

#[test]
fn snowflake_function_wrap_stays_unknown() {
    let mut f = fixture();
    let a = f.cols[1];
    let pred = f.wrapped(a, "to_date");
    let (mir, phys) = f.filtered(vec![pred]);
    let lir = lower(&mir, Backend::Snowflake, &phys);
    // the partition key is not a Snowflake key kind, so only clusters remain
    assert_eq!(usages(&lir)[0], (KeyKind::Cluster, PruneUse::Unknown));
}

#[test]
fn keys_a_backend_cannot_use_are_not_stated() {
    let (mir, phys) = fixture().scanned();
    assert!(usages(&lower(&mir, Backend::Generic, &phys)).is_empty());
    assert!(usages(&lower(&mir, Backend::DuckDb, &phys)).is_empty());
}

#[test]
fn estimates_come_only_from_given_stats() {
    let (mir, phys) = fixture().scanned();
    let lir = lower(&mir, Backend::Generic, &phys);
    assert_eq!(lir.nodes[lir.root].est, Estimate::Stats { rows: 1000 });
    let lir = lower(&mir, Backend::Generic, &PhysicalFacts::default());
    assert_eq!(lir.nodes[lir.root].est, Estimate::Unknown);
}

#[test]
fn sink_resolves_config_names_against_root_columns() {
    let (mut mir, phys) = fixture().scanned();
    let (ts, ghost) = (mir.syms.intern("ts"), mir.syms.intern("ghost"));
    mir.config.materialized = Some(Materialization::Incremental);
    mir.config.partition_by = vec![ts, ghost];
    mir.config.contract_enforced = true;
    let lir = lower(&mir, Backend::BigQuery, &phys);
    assert_eq!(lir.sink.mat, Materialization::Incremental);
    assert_eq!(
        lir.sink.partition_by.len(),
        1,
        "an unknown name is not stated"
    );
    assert!(lir.sink.contract);
}

#[test]
fn an_absent_materialization_is_dbts_default_view() {
    let (mir, phys) = fixture().scanned();
    assert_eq!(
        lower(&mir, Backend::Generic, &phys).sink.mat,
        Materialization::View
    );
}

fn cte_plan() -> Mir {
    let mut b = MirBuilder::new();
    let scan = b.scan("t", &[("a", int_col())]);
    let cte = b.cte("c", scan);
    let (x, y) = (b.cte_ref(cte), b.cte_ref(cte));
    let root = b.join(JoinKind::Cross, x, y, vec![]);
    b.finish(root)
}

fn first_cte(lir: &Lir) -> (CtePolicy, u32) {
    lir.nodes
        .values()
        .find_map(|n| match n.op {
            PhysOp::Cte { policy, uses, .. } => Some((policy, uses)),
            _ => None,
        })
        .expect("a cte")
}

#[test]
fn cte_use_counts_and_policy_follow_the_backend() {
    let mir = cte_plan();
    let phys = PhysicalFacts::default();
    assert_eq!(
        first_cte(&lower(&mir, Backend::Postgres, &phys)),
        (CtePolicy::Materialize, 2)
    );
    assert_eq!(
        first_cte(&lower(&mir, Backend::BigQuery, &phys)),
        (CtePolicy::Inline, 2)
    );
    assert_eq!(
        first_cte(&lower(&mir, Backend::DuckDb, &phys)),
        (CtePolicy::Auto, 2)
    );
}

#[test]
fn joins_carry_no_hint() {
    let mir = cte_plan();
    let lir = lower(&mir, Backend::Snowflake, &PhysicalFacts::default());
    let hint = lir.nodes.values().find_map(|n| match n.op {
        PhysOp::Join { hint, build, .. } => Some((hint, build)),
        _ => None,
    });
    assert_eq!(hint, Some((JoinHint::None, None)));
}

#[test]
fn union_lowers_and_intersect_is_opaque() {
    let build = |op: SetOpKind| {
        let mut b = MirBuilder::new();
        let (l, r) = (
            b.scan("l", &[("a", int_col())]),
            b.scan("r", &[("a", int_col())]),
        );
        let out = b.col("a", int_col());
        let root = b.set_op(op, true, vec![l, r], vec![out]);
        b.finish(root)
    };
    let phys = PhysicalFacts::default();
    let union = build(SetOpKind::Union);
    let lir = lower(&union, Backend::Generic, &phys);
    assert!(matches!(
        lir.nodes[lir.root].op,
        PhysOp::Union { all: true, .. }
    ));
    let intersect = build(SetOpKind::Intersect);
    let lir = lower(&intersect, Backend::Generic, &phys);
    assert!(matches!(lir.nodes[lir.root].op, PhysOp::Opaque { .. }));
}

#[test]
fn distinct_is_a_key_only_aggregate_when_key_scalars_exist() {
    let mut b = MirBuilder::new();
    let scan = b.scan("t", &[("a", int_col())]);
    let col = b.out(scan)[0];
    b.col_ref(col);
    let root = b.distinct(scan);
    let mir = b.finish(root);
    let lir = lower(&mir, Backend::Generic, &PhysicalFacts::default());
    assert!(
        matches!(&lir.nodes[lir.root].op, PhysOp::Aggregate { keys, aggs, .. } if keys.len() == 1 && aggs.is_empty())
    );
}

#[test]
fn an_opaque_mir_node_makes_the_level_partial() {
    let mut b = MirBuilder::new();
    let col = b.col("a", int_col());
    let root = b.opaque(vec![], vec![col], HoleKind::ParseError);
    let mir = b.finish(root);
    let lowered = lower_to_lir(&mir, backend(Backend::Generic), &PhysicalFacts::default());
    lowered.assert_clean();
    assert_eq!(lowered.fidelity, Fidelity::Partial);
}

#[test]
fn invalid_mir_declines_instead_of_panicking() {
    let (mut mir, phys) = fixture().scanned();
    let scan = mir.root;
    let dup = mir.rels[scan].op.clone();
    mir.rels[scan].op = match dup {
        RelOp::Scan {
            table,
            mut cols,
            open,
        } => {
            cols.push(cols[0]);
            RelOp::Scan { table, cols, open }
        }
        other => other,
    };
    let lowered = lower_to_lir(&mir, backend(Backend::Generic), &phys);
    assert!(lowered.value.is_none());
    assert_eq!(lowered.fidelity, Fidelity::Declined);
    assert!(lowered
        .violations
        .iter()
        .any(|v| v.invariant.level() == cogitator_ir::Level::Mir));
}

#[test]
fn every_backend_passes_the_lir_verifier_on_one_plan() {
    let mut f = fixture();
    let ts = f.cols[0];
    let pred = f.cmp(ts, BinaryOp::GtEq);
    let (mir, phys) = f.filtered(vec![pred]);
    for b in [
        Backend::Snowflake,
        Backend::BigQuery,
        Backend::DuckDb,
        Backend::Postgres,
        Backend::Generic,
    ] {
        let lir = lower(&mir, b, &phys);
        assert_eq!(lir.backend, b);
    }
}

#[test]
fn stats_are_keyed_by_table_id() {
    let f = fixture();
    assert!(f.phys.table(TableId::from_raw(0)).is_some());
    assert!(matches!(
        f.b.mir.scalars.values().next().map(|s| &s.kind),
        None | Some(ScalarKind::Col(_))
    ));
}
