//! Column types over `testdata/type_check`, through the real schema job:
//! casts, literals, joins, unions, aggregates, upstream evidence.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use cogitator_dialects::Dialect;
use cogitator_ir::types::SqlType;
use rstest::rstest;

use super::*;
use crate::features::plan::schemas::{
    run_schema_job, JobInput, JobLimits, JobState, SavedFiles, SchemaSnapshot,
};
use crate::{build_context, DbtContext};

pub(super) fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/type_check")
}

struct Disk;

impl SavedFiles for Disk {
    fn stamp(&self, path: &Path) -> Option<u64> {
        path.is_file().then_some(0)
    }
    fn read(&self, path: &Path) -> Option<String> {
        std::fs::read_to_string(path).ok()
    }
}

/// The fixture's context (duckdb) and its schema snapshot.
pub(super) fn project() -> (DbtContext, Arc<SchemaSnapshot>) {
    let mut ctx = build_context(fixture_root().to_str().expect("utf-8 path"));
    ctx.dialect = Dialect::new("duckdb");
    let never = || false;
    let input = JobInput {
        dialect: &ctx.dialect,
        models: &ctx.model_detail_map,
        sources: &ctx.source_detail_map,
        graph: &ctx.graph,
        manifest: None,
        catalog: None,
        files: &Disk,
        limits: JobLimits::default(),
        stale: &never,
    };
    let snapshot = run_schema_job(&input, &mut JobState::default()).expect("not superseded");
    (ctx, snapshot)
}

fn col(model: &str, name: &str) -> ColumnType {
    let (ctx, snapshot) = project();
    column_types(&ctx, &snapshot, model)
        .unwrap_or_else(|| panic!("{model} has inferred types"))
        .column(name)
        .unwrap_or_else(|| panic!("{model}.{name}"))
        .clone()
}

#[rstest]
#[case("stg_orders", "order_id", SqlType::Int { bits: Some(64) }, Basis::Proven, Some("bigint"))]
#[case("stg_orders", "amount", SqlType::Decimal { p: Some(10), s: Some(2) }, Basis::Proven, Some("numeric(10,2)"))]
#[case("stg_orders", "ordered_on", SqlType::Date, Basis::Proven, Some("date"))]
#[case("stg_orders", "channel", SqlType::Text, Basis::Proven, Some("varchar"))]
// an untyped source column claims nothing
#[case("stg_orders", "status", SqlType::Unknown, Basis::Unknown, None)]
// through a ref, typed by the upstream's own SQL
#[case("order_stats", "order_id", SqlType::Int { bits: Some(64) }, Basis::Proven, Some("bigint"))]
#[case("order_stats", "order_count", SqlType::Int { bits: None }, Basis::Proven, None)]
#[case(
    "order_stats",
    "last_order",
    SqlType::Date,
    Basis::Proven,
    Some("date")
)]
#[case(
    "order_stats",
    "channel_code",
    SqlType::Text,
    Basis::Proven,
    Some("varchar")
)]
// resting on a source's YAML data_type: shown, not proven
#[case("legacy_codes", "code", SqlType::Int { bits: Some(32) }, Basis::Declared, Some("integer"))]
#[case("order_stats", "code", SqlType::Int { bits: Some(32) }, Basis::Declared, Some("integer"))]
// union arms widen
#[case("unioned", "label", SqlType::Text, Basis::Proven, Some("varchar"))]
fn inferred_types(
    #[case] model: &str,
    #[case] name: &str,
    #[case] ty: SqlType,
    #[case] basis: Basis,
    #[case] spelled: Option<&str>,
) {
    let c = col(model, name);
    assert_eq!(c.sql_type, ty, "{model}.{name}");
    assert_eq!(c.basis, basis, "{model}.{name}");
    assert_eq!(c.data_type.as_deref(), spelled, "{model}.{name}");
}

#[rstest]
#[case("stg_orders", "order_id", Provenance::Cast)]
#[case("stg_orders", "channel", Provenance::Literal)]
#[case("order_stats", "order_count", Provenance::Aggregate { name: "count".into() })]
#[case("order_stats", "channel_code", Provenance::Function { name: "upper".into() })]
#[case("order_stats", "doubled", Provenance::Expression)]
#[case("unioned", "n", Provenance::SetOp)]
fn provenances(#[case] model: &str, #[case] name: &str, #[case] want: Provenance) {
    assert_eq!(col(model, name).provenance, want);
}

#[test]
fn a_passed_through_column_names_its_upstream() {
    let c = col("order_stats", "order_id");
    let Provenance::Upstream { from } = &c.provenance else {
        panic!("{:?}", c.provenance);
    };
    assert_eq!(
        from.relation,
        Relation::Model {
            name: "stg_orders".into()
        }
    );
    assert_eq!(c.feeds[0].origin, Origin::Inferred);
    assert!(c.why.contains("stg_orders.order_id"), "{}", c.why);
}

#[test]
fn count_is_never_null_and_needs_no_upstream() {
    let c = col("order_stats", "order_count");
    assert_eq!(c.nullable, Some(false));
    assert!(c.feeds.is_empty());
}

#[test]
fn unions_widen_numbers() {
    let c = col("unioned", "n");
    assert!(
        matches!(c.sql_type, SqlType::Decimal { .. }),
        "{:?}",
        c.sql_type
    );
    assert_eq!(c.data_type, None);
}

#[test]
fn a_model_the_job_could_not_infer_has_no_types() {
    let (ctx, snapshot) = project();
    assert!(column_types(&ctx, &snapshot, "no_such_model").is_none());
}
