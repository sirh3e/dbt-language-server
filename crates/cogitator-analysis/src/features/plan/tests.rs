use std::collections::HashMap;

use cogitator_dialects::Dialect;
use cogitator_ir::lir::{
    Backend, Capabilities, JoinHint, Lir, PhysColumn, PhysicalFacts, Pred, Pruning,
};
use cogitator_ir::mir::Mir;
use cogitator_ir::{Fidelity, Invariant, Lowered, Violation};
use cogitator_lower::{lower_plan, lower_to_mir, DialectBackend};
use cogitator_schemas::ColumnEntry;

use super::*;
use crate::artifacts::catalog::{CatalogColumn, CatalogIndex, CatalogTable};
use crate::artifacts::manifest::{ManifestConfig, ManifestIndex, ManifestNode};

/// Owns what a `PlanInput` borrows.
struct Facts {
    dialect: Dialect,
    models: HashMap<String, ModelDetails>,
    sources: HashMap<String, SourceDetails>,
    manifest: Option<ManifestIndex>,
    catalog: Option<CatalogIndex>,
}

impl Facts {
    fn new(dialect: &str) -> Self {
        Facts {
            dialect: Dialect::new(dialect),
            models: HashMap::new(),
            sources: HashMap::new(),
            manifest: None,
            catalog: None,
        }
    }

    fn yaml(mut self, model: &str, columns: &[(&str, Option<&str>)]) -> Self {
        self.models.insert(
            model.to_string(),
            ModelDetails {
                columns: columns.iter().map(|(name, ty)| column(name, *ty)).collect(),
                ..ModelDetails::default()
            },
        );
        self
    }

    fn manifest(mut self, model: &str, config: ManifestConfig) -> Self {
        self.manifest
            .get_or_insert_with(ManifestIndex::default)
            .models
            .insert(
                model.to_string(),
                ManifestNode {
                    unique_id: format!("model.p.{model}"),
                    original_file_path: None,
                    relation_name: None,
                    config,
                },
            );
        self
    }

    fn input<'a>(&'a self, text: &'a str, model: Option<&'a str>) -> PlanInput<'a> {
        PlanInput {
            text,
            dialect: &self.dialect,
            model,
            models: &self.models,
            sources: &self.sources,
            manifest: self.manifest.as_ref(),
            catalog: self.catalog.as_ref(),
            schema_revision: 7,
        }
    }

    fn plan(&self, text: &str) -> QueryPlan {
        query_plan(&self.input(text, Some("m")))
    }
}

fn column(name: &str, data_type: Option<&str>) -> ColumnEntry {
    ColumnEntry {
        name: name.to_string(),
        description: String::new(),
        name_pos: Default::default(),
        data_type: data_type.map(str::to_string),
    }
}

fn rows_for<'a>(plan: &'a QueryPlan, rule: &str) -> Vec<&'a TraceRow> {
    plan.trace
        .entries
        .iter()
        .filter(|r| r.rule == rule)
        .collect()
}

#[test]
fn a_plain_model_renders_every_level_and_the_trace() {
    let plan = Facts::new("duckdb").plan(
        "with a as (select id, amount from {{ ref('up') }}) \
         select id from a where amount > 1",
    );
    assert_eq!(plan.status, PlanStatus::Complete, "{plan:#?}");
    assert!(plan.guidance.is_none());
    assert_eq!(plan.backend, Backend::DuckDb);
    assert_eq!(plan.schema_revision, 7);
    assert_eq!(
        plan.origin,
        Some(OriginView::Template {
            variant: cogitator_ir::origin::TemplateVariant::Plain
        })
    );
    for level in [&plan.levels.hir, &plan.levels.mir, &plan.levels.lir] {
        assert!(level.available, "{level:#?}");
        assert_eq!(level.fidelity, Some(Fidelity::Complete));
        assert!(level.text.as_deref().is_some_and(|t| !t.is_empty()));
    }
    assert!(!plan.trace.entries.is_empty());
    assert!(!plan.trace.incomplete, "{:#?}", plan.trace);
    // canonicalisers ran and were recorded
    assert!(!rows_for(&plan, "canon.simplify").is_empty());
}

#[test]
fn unmet_needs_are_skipped_with_a_reason() {
    let plan = Facts::new("duckdb").plan("select id from {{ ref('up') }} where date(ts) = d");
    assert!(plan.needs.missing.contains(&"phys"));
    assert!(plan.needs.missing.contains(&"schema"));
    assert!(plan.needs.available.contains(&"dialect"));
    assert!(plan.needs.available.contains(&"allBranches"));
    let rows = rows_for(&plan, "filter.non_sargable");
    assert_eq!(rows.len(), 1, "{rows:#?}");
    assert_eq!(rows[0].outcome, "skipped");
    assert_eq!(rows[0].reason.as_deref(), Some("unmet:types"));
}

#[test]
fn a_model_with_a_hole_still_produces_levels() {
    let plan = Facts::new("duckdb").plan("with a as (select from from where) select id from a");
    assert_eq!(plan.status, PlanStatus::Partial, "{plan:#?}");
    assert!(plan.levels.hir.available);
    assert!(
        !plan.levels.hir.degradations.is_empty(),
        "{:#?}",
        plan.levels.hir
    );
    assert!(plan.levels.mir.available, "{:#?}", plan.levels.mir);
    assert!(plan.levels.lir.available, "{:#?}", plan.levels.lir);
}

#[test]
fn control_flow_declines_with_guidance_and_skips_every_rule() {
    let plan = Facts::new("duckdb")
        .plan("select {% for c in cols %}{{ c }},{% endfor %} 1 from {{ ref('up') }}");
    assert_eq!(plan.status, PlanStatus::Declined);
    let guidance = plan.guidance.as_deref().expect("guidance");
    assert!(guidance.contains("dbt compile"), "{guidance}");
    assert!(!plan.levels.hir.available);
    assert!(plan
        .levels
        .hir
        .unavailable
        .as_deref()
        .is_some_and(|n| n.contains("declined")));
    assert!(!plan.levels.mir.available && !plan.levels.lir.available);
    assert!(plan.trace.incomplete);
    assert!(plan
        .trace
        .entries
        .iter()
        .all(|r| r.outcome == "skipped" && r.reason.as_deref() == Some("unavailable:hir")));
    assert!(!plan.trace.entries.is_empty());
}

#[test]
fn a_lone_is_incremental_block_plans_the_incremental_variant() {
    let plan = Facts::new("duckdb").plan(
        "select id from {{ ref('up') }}\n\
         {% if is_incremental() %}where id > (select max(id) from {{ this }}){% endif %}",
    );
    assert_ne!(plan.status, PlanStatus::Declined, "{plan:#?}");
    assert_eq!(
        plan.origin,
        Some(OriginView::Template {
            variant: cogitator_ir::origin::TemplateVariant::Incremental
        })
    );
    assert!(plan.levels.lir.available);
}

#[test]
fn a_forced_mir_violation_drops_mir_and_lir_but_not_hir() {
    let facts = Facts::new("duckdb");
    let input = facts.input("select 1 as x", Some("m"));
    let forced = |_: &cogitator_ir::hir::Hir| -> Lowered<Mir> {
        Lowered::declined(Vec::new())
            .with_violations(vec![Violation::new(Invariant::MirIdRange, "forced")])
    };
    let backend = cogitator_lower::backends::for_backend(Backend::DuckDb).expect("backend");
    let plan = drive::drive(&input, backend, &forced);
    assert_eq!(plan.status, PlanStatus::Partial);
    assert!(plan.levels.hir.available);
    assert!(!plan.levels.mir.available);
    let note = plan.levels.mir.unavailable.as_deref().expect("note");
    assert!(note.contains("MirIdRange"), "{note}");
    assert_eq!(plan.levels.mir.violations.len(), 1);
    assert!(!plan.levels.lir.available);
    assert!(plan
        .trace
        .entries
        .iter()
        .all(|r| r.reason.as_deref() == Some("unavailable:mir")));
}

/// A backend whose capability table permits no CTE policy at all, so the
/// `Auto` every CTE lowers to must fail `verify_lir`.
struct NoCtePolicies;

static NO_CTE_CAPS: Capabilities = Capabilities {
    backend: Backend::Generic,
    join_hints: &[JoinHint::None],
    cte_policies: &[],
    pruning_keys: &[],
    doc: "",
};

impl DialectBackend for NoCtePolicies {
    fn capabilities(&self) -> &'static Capabilities {
        &NO_CTE_CAPS
    }
    fn lower(&self, mir: &Mir, phys: &PhysicalFacts) -> Lir {
        lower_plan(mir, phys, self)
    }
    fn pruning(&self, _pred: &Pred, _col: &PhysColumn) -> Pruning {
        Pruning::Unknown
    }
}

#[test]
fn a_forced_lir_violation_drops_only_lir() {
    let facts = Facts::new("duckdb");
    let input = facts.input("with a as (select 1 as x) select x from a", Some("m"));
    let plan = drive::drive(&input, &NoCtePolicies, &lower_to_mir);
    assert!(plan.levels.hir.available);
    assert!(plan.levels.mir.available, "{:#?}", plan.levels.mir);
    assert!(!plan.levels.lir.available, "{:#?}", plan.levels.lir);
    assert!(plan
        .levels
        .lir
        .violations
        .iter()
        .any(|v| v.invariant == Invariant::LirHintSupported));
    assert!(plan
        .levels
        .lir
        .unavailable
        .as_deref()
        .is_some_and(|n| n.contains("LirHintSupported")));
    assert_eq!(plan.status, PlanStatus::Partial);
    // the rules that read LIR say so instead of disappearing
    assert!(plan
        .trace
        .entries
        .iter()
        .any(|r| r.phase == "detectLir" && r.reason.as_deref() == Some("unavailable:lir")));
}

#[test]
fn yaml_columns_expand_a_star_and_stay_open() {
    let facts = Facts::new("duckdb").yaml("up", &[("id", Some("integer")), ("name", None)]);
    let plan = facts.plan("select * from {{ ref('up') }}");
    let mir = plan.levels.mir.text.as_deref().expect("mir");
    assert!(mir.contains("id") && mir.contains("name"), "{mir}");
    assert!(plan.needs.available.contains(&"schema"));
    assert!(plan.needs.available.contains(&"types"));
}

#[test]
fn a_partly_known_upstream_set_is_not_a_known_schema() {
    let facts = Facts::new("duckdb").yaml("up", &[("id", None)]);
    let plan =
        facts.plan("select u.id from {{ ref('up') }} u join {{ ref('other') }} o on u.id = o.id");
    assert!(plan.needs.missing.contains(&"schema"), "{:#?}", plan.needs);
    assert!(plan.needs.missing.contains(&"types"));
}

#[test]
fn manifest_config_fills_gaps_the_template_left() {
    let facts = Facts::new("bigquery").manifest(
        "m",
        ManifestConfig {
            materialized: Some("incremental".to_string()),
            partition_by: Some(serde_json::json!({"field": "day"})),
            ..ManifestConfig::default()
        },
    );
    let plan = facts.plan("select day, id from {{ ref('up') }}");
    assert!(plan.needs.available.contains(&"mat"));
    let lir = plan.levels.lir.text.as_deref().expect("lir");
    assert!(
        lir.contains("incremental") || lir.contains("Incremental"),
        "{lir}"
    );
    assert!(lir.contains("day"), "{lir}");
}

#[test]
fn the_templates_config_beats_the_manifest() {
    let facts = Facts::new("duckdb").manifest(
        "m",
        ManifestConfig {
            materialized: Some("table".to_string()),
            ..ManifestConfig::default()
        },
    );
    let plan = facts.plan("{{ config(materialized='view') }} select 1 as x");
    let lir = plan.levels.lir.text.as_deref().expect("lir");
    assert!(
        lir.to_lowercase().contains("view"),
        "the file's own config wins: {lir}"
    );
}

#[test]
fn physical_facts_come_from_the_manifest_and_catalog() {
    let mut facts = Facts::new("snowflake").manifest(
        "up",
        ManifestConfig {
            cluster_by: vec!["to_date(ts)".to_string()],
            ..ManifestConfig::default()
        },
    );
    let mut catalog = CatalogIndex::default();
    let mut table = CatalogTable {
        columns: vec![CatalogColumn {
            name: "A".to_string(),
            data_type: "NUMBER".to_string(),
            comment: None,
            index: Some(1),
        }],
        ..CatalogTable::default()
    };
    table.stats.row_count = Some(10);
    table.stats.clustering = Some("LINEAR(a, b)".to_string());
    catalog.models.insert("src_up".to_string(), table);
    facts.catalog = Some(catalog);

    let plan = facts.plan("select ts from {{ ref('up') }} where ts > '2024-01-01'");
    assert!(plan.needs.available.contains(&"phys"), "{:#?}", plan.needs);
    let lir = plan.levels.lir.text.as_deref().expect("lir");
    assert!(lir.contains("TS") || lir.contains("ts"), "{lir}");

    let stats = facts.plan("select a, b from {{ ref('src_up') }} where a > 1");
    assert!(
        stats.needs.available.contains(&"stats"),
        "{:#?}",
        stats.needs
    );
    assert!(
        stats.needs.available.contains(&"phys"),
        "{:#?}",
        stats.needs
    );
}

#[test]
fn an_unknown_dialect_is_generic_and_lacks_the_dialect_need() {
    let plan = Facts::new("redshift").plan("select 1 as x");
    assert_eq!(plan.backend, Backend::Generic);
    assert!(plan.needs.missing.contains(&"dialect"));
}

#[test]
fn the_plan_is_deterministic_json() {
    let facts = Facts::new("duckdb").yaml("up", &[("id", Some("int"))]);
    let text = "with a as (select id from {{ ref('up') }}) select id from a";
    let once = serde_json::to_string(&facts.plan(text)).expect("serialize");
    let twice = serde_json::to_string(&facts.plan(text)).expect("serialize");
    assert_eq!(once, twice);
    let value: serde_json::Value = serde_json::from_str(&once).expect("json");
    assert_eq!(value["levels"]["hir"]["available"], true);
    assert_eq!(value["schemaRevision"], 7);
    assert_eq!(value["status"], "complete");
}

/// Named columns of an open upstream, read through a `SELECT *` CTE, must
/// survive as columns rather than becoming opaque (demand tracking).
#[test]
fn bound_columns_flow_through_an_open_star_cte() {
    let facts = Facts::new("duckdb").yaml("up", &[("day", None), ("id", None)]);
    let plan = facts.plan("with u as (select * from {{ ref('up') }}) select id, day from u");
    assert!(
        plan.levels
            .mir
            .degradations
            .iter()
            .all(|d| !d.detail.contains("no such column")),
        "{:#?}",
        plan.levels.mir
    );
    let mir = plan.levels.mir.text.as_deref().expect("mir");
    assert!(!mir.contains("opaque["), "{mir}");
    assert!(
        mir.contains("cte#0 u out=[") && mir.contains("day"),
        "{mir}"
    );
}

/// Fixtures and awkward shapes run the whole pipeline with no verifier
/// violation and no rollback, whatever fidelity they land on.
#[test]
fn every_fixture_and_shape_plans_without_a_violation() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
    let mut corpus: Vec<(String, String)> = walkdir::WalkDir::new(&root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "sql"))
        .map(|e| {
            let text = std::fs::read_to_string(e.path()).expect("fixture");
            (e.path().display().to_string(), text)
        })
        .collect();
    assert!(!corpus.is_empty());
    corpus.sort();
    for sql in [
        "select a, sum(b) over (partition by c order by d) as s from {{ ref('t') }} qualify s > 1",
        "select * from {{ ref('a') }} union all select * from {{ ref('b') }}",
        "select a from {{ ref('a') }} except select a from {{ ref('b') }} order by 1 limit 3",
        "select x.a, (select max(b) from {{ ref('b') }} b where b.id = x.id) m from {{ ref('a') }} x",
        "select a, a + 1 as c, c * 2 as d from {{ ref('t') }} group by all",
        "select u.value from {{ ref('t') }} t, unnest(t.arr) as u(value)",
        "select * exclude (a) replace (b + 1 as b) from {{ ref('t') }}",
        "select count(*) filter (where a > 1) from {{ source('s', 't') }} left join {{ ref('x') }} using (id)",
        "with recursive r as (select 1 as n union all select n + 1 from r where n < 5) select n from r",
        "with a as (select 1 x), b as (select * from a), c as (select x from a join b using (x)) select x from c",
        "select distinct a from t1 cross join t2 where t1.id in (select id from t3) and exists (select 1 from t4)",
        "select {{ my_macro('a') }} as x, {{ var('y') }} from {{ dbt_utils.star(ref('t')) }}",
        "select {{ col }} from {{ this }} where {{ cond }}",
        "{{ config(materialized='incremental', unique_key='id') }} select id from {{ ref('t') }} \
         {% if is_incremental() %} where id > (select max(id) from {{ this }}) {% endif %}",
        "select a from (select a from (select 1 as a) s1) s2 where a = 1 and 1 = 1",
        "select from",
        "",
        "-- comment only\n",
        "select a,, b from t",
        "with a as (selec 1) select * from a",
    ] {
        corpus.push(("shape".to_string(), sql.to_string()));
    }
    let facts = Facts::new("duckdb").yaml("a", &[("a", None), ("b", None)]);
    for (name, text) in &corpus {
        let plan = facts.plan(text);
        for level in [&plan.levels.hir, &plan.levels.mir, &plan.levels.lir] {
            assert!(level.violations.is_empty(), "{name}: {level:#?}");
        }
        assert!(
            plan.trace.entries.iter().all(|r| r.outcome != "rolledBack"),
            "{name}: {:#?}",
            plan.trace
        );
    }
}
