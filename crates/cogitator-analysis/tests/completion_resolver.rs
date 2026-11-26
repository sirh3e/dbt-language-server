//! The column name resolver behind column completion, over
//! `testdata/column_resolver` and the real schema job: a model's columns
//! resolved through the project (an undocumented `select *` of an
//! undocumented model, a CTE's `w.*`), with their types and YAML
//! descriptions, and what completion knew without the snapshot.

use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use cogitator_analysis::features::plan::schemas::{
    run_schema_job, JobInput, JobLimits, JobState, SavedFiles, SchemaSnapshot,
};
use cogitator_analysis::{build_context, features, DbtContext, Documents};
use cogitator_dialects::Dialect;
use cogitator_lsp::{completion_kind, CompletionItem, Position};
use pretty_assertions::assert_eq;
use rstest::rstest;

struct Disk;

impl SavedFiles for Disk {
    fn stamp(&self, path: &Path) -> Option<u64> {
        path.is_file().then_some(0)
    }
    fn read(&self, path: &Path) -> Option<String> {
        std::fs::read_to_string(path).ok()
    }
}

static PROJECT: LazyLock<(DbtContext, Arc<SchemaSnapshot>)> = LazyLock::new(|| {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/column_resolver");
    let mut ctx = build_context(root.to_str().expect("utf-8 path"));
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
});

/// The column items at the `|`, in the order a client shows them; with the
/// schema snapshot when `resolved`.
fn columns(marked: &str, resolved: bool) -> Vec<CompletionItem> {
    let (ctx, snapshot) = &*PROJECT;
    let at = marked.find('|').expect("cursor mark");
    let text = marked.replacen('|', "", 1);
    let line = text[..at].matches('\n').count() as u32;
    let character = (at - text[..at].rfind('\n').map_or(0, |i| i + 1)) as u32;
    let uri = "file:///tmp/resolver.sql";
    let mut docs = Documents::default();
    docs.open(uri.to_string(), &text, &ctx.dialect);
    let read = |p: &Path| std::fs::read_to_string(p).ok();
    let mut items = features::completion_resolving(
        ctx,
        docs.get(uri).expect("opened"),
        Position { line, character },
        &read,
        resolved.then(|| &*snapshot.schemas),
    );
    items.retain(|i| i.kind == completion_kind::FIELD);
    items.sort_by(|a, b| a.sort_text.cmp(&b.sort_text));
    items
}

fn shown(items: &[CompletionItem]) -> Vec<(&str, &str)> {
    items
        .iter()
        .map(|i| (i.label.as_str(), i.detail.as_str()))
        .collect()
}

#[rstest]
// `select *` of an undocumented model: its upstream's columns, with the
// types its SQL proves
#[case::star_of_undocumented(
    "select w.| from {{ ref('wide') }} w",
    &[("customer_id", "wide · bigint"), ("name", "wide"), ("signed_up", "wide · date")],
)]
// two `*`s deep, through a CTE's `w.*`, and a column of its own
#[case::star_through_a_cte(
    "select e.| from {{ ref('enriched') }} e",
    &[
        ("customer_id", "enriched · bigint"),
        ("name", "enriched"),
        ("signed_up", "enriched · date"),
        ("active", "enriched · boolean"),
    ],
)]
// a model whose SQL names its columns, though no YAML documents it
#[case::undocumented_model(
    "select s.| from {{ ref('stg_customers') }} s",
    &[
        ("customer_id", "stg_customers · bigint"),
        ("name", "stg_customers"),
        ("signed_up", "stg_customers · date"),
    ],
)]
// a source table: its YAML's columns, typed where the YAML says
#[case::source(
    "select c.| from {{ source('shop', 'customers') }} c",
    &[("id", "shop.customers · integer"), ("name", "shop.customers"), ("signed_up", "shop.customers")],
)]
fn the_resolver_knows_what_a_relation_has(#[case] marked: &str, #[case] want: &[(&str, &str)]) {
    assert_eq!(shown(&columns(marked, true)), want, "{marked}");
}

#[test]
fn a_column_keeps_its_yaml_description() {
    let items = columns("select e.| from {{ ref('enriched') }} e", true);
    let active = items.iter().find(|i| i.label == "active").expect("active");
    assert_eq!(
        active.documentation,
        "Whether the customer still has an account."
    );
    let items = columns("select c.| from {{ source('shop', 'customers') }} c", true);
    let name = items.iter().find(|i| i.label == "name").expect("name");
    assert_eq!(name.documentation, "The customer's display name.");
}

/// Without the snapshot, a model that is only `select *` of another
/// offered nothing: its own lineage cannot expand the star. The resolver is
/// what closes that.
#[test]
fn without_the_snapshot_a_star_of_an_undocumented_model_offers_nothing() {
    assert_eq!(
        shown(&columns("select w.| from {{ ref('wide') }} w", false)),
        []
    );
}

/// Bare columns in a select list come from the same resolver.
#[test]
fn a_select_list_offers_the_resolved_columns() {
    let items = columns("select | from {{ ref('wide') }}", true);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    for column in ["customer_id", "name", "signed_up"] {
        assert!(labels.contains(&column), "{column} in {labels:?}");
    }
}
