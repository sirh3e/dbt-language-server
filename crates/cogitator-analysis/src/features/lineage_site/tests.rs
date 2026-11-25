//! The lineage site on the jaffle fixture: the DAG is `dbt.projectGraph`'s,
//! a known column chain, declined models, stable output, the page.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cogitator_dialects::Dialect;

use super::*;
use crate::artifacts::ArtifactSet;
use crate::features::plan::schemas::{run_schema_job, JobInput, JobLimits, JobState, SavedFiles};
use crate::features::plan::CompiledInput;
use crate::features::project_graph_json;
use crate::{build_context, selectors};

struct Disk;

impl SavedFiles for Disk {
    fn stamp(&self, path: &Path) -> Option<u64> {
        path.is_file().then_some(0)
    }
    fn read(&self, path: &Path) -> Option<String> {
        std::fs::read_to_string(path).ok()
    }
}

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/jaffle_shop_duckdb")
}

fn context() -> DbtContext {
    let mut ctx = build_context(fixture().to_str().expect("utf-8 path"));
    ctx.dialect = Dialect::new("duckdb");
    ctx
}

fn site_of(ctx: &DbtContext) -> LineageSite {
    let never = || false;
    let snapshot = run_schema_job(
        &JobInput {
            dialect: &ctx.dialect,
            models: &ctx.model_detail_map,
            sources: &ctx.source_detail_map,
            graph: &ctx.graph,
            manifest: None,
            catalog: None,
            files: &Disk,
            limits: JobLimits::default(),
            stale: &never,
        },
        &mut JobState::default(),
    );
    let read = |p: &Path| std::fs::read_to_string(p).ok();
    let universe = selectors::universe(ctx, &read);
    let artifacts = ArtifactSet::default();
    let project: Option<Arc<_>> = snapshot;
    let traced = |model: &str| {
        let text = ctx
            .model_detail_map
            .get(model)
            .and_then(|d| std::fs::read_to_string(&d.uri).ok());
        trace_model(
            ctx,
            &artifacts,
            project.as_deref(),
            ModelText {
                model,
                text: text.as_deref(),
                compiled: CompiledInput::NotChecked,
            },
        )
    };
    export(&SiteInput {
        ctx,
        universe: &universe,
        schemas: project.as_deref(),
        traced: &traced,
        generator: "cogitator test",
    })
}

fn node<'a>(site: &'a LineageSite, id: &str) -> &'a SiteNode {
    site.nodes
        .iter()
        .find(|n| n.id == id)
        .unwrap_or_else(|| panic!("no node {id}"))
}

fn column<'a>(node: &'a SiteNode, name: &str) -> &'a SiteColumn {
    node.columns
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("no column {name} in {}", node.id))
}

#[test]
fn nodes_and_edges_are_the_project_graphs() {
    let ctx = context();
    let site = site_of(&ctx);
    let graph = project_graph_json(&ctx);
    let site_nodes: BTreeSet<(&str, usize)> = site
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.layer))
        .collect();
    let graph_nodes: BTreeSet<(&str, usize)> = graph
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.layer))
        .collect();
    assert_eq!(site_nodes, graph_nodes);
    let site_edges: BTreeSet<_> = site
        .edges
        .iter()
        .map(|e| (e.parent.clone(), e.child.clone()))
        .collect();
    let graph_edges: BTreeSet<_> = graph
        .edges
        .iter()
        .map(|e| (e.from.clone(), e.to.clone()))
        .collect();
    assert_eq!(site_edges, graph_edges);
    // dependency → dependent: the seed feeds its staging model
    assert!(site
        .edges
        .iter()
        .any(|e| e.parent == "model:raw_customers" && e.child == "model:stg_customers"));
    assert_eq!(site.counts.nodes, site.nodes.len());
    assert_eq!(site.counts.edges, site.edges.len());
}

#[test]
fn kinds_details_and_tests_come_from_the_project() {
    let site = site_of(&context());
    assert_eq!(node(&site, "model:raw_customers").kind, SiteKind::Seed);
    let customers = node(&site, "model:customers");
    assert_eq!(customers.kind, SiteKind::Model);
    assert_eq!(customers.materialized.as_deref(), Some("table"));
    assert_eq!(customers.path.as_deref(), Some("models/customers.sql"));
    assert!(
        customers.tags.contains(&"nightly".to_string()),
        "{customers:?}"
    );
    assert!(
        customers
            .tests
            .contains(&"unique_customers_customer_id".to_string()),
        "{:?}",
        customers.tests
    );
    let stg = node(&site, "model:stg_customers");
    assert_eq!(stg.materialized.as_deref(), Some("view"));
    let id = column(customers, "customer_id");
    assert!(id.documented && id.produced, "{id:?}");
    assert!(id.tests.contains(&"unique".to_string()), "{id:?}");
}

#[test]
fn a_renamed_column_is_followed_to_the_seed() {
    let site = site_of(&context());
    let stg = node(&site, "model:stg_customers");
    assert_eq!(stg.lineage, LineageState::Traced);
    let id = column(stg, "customer_id");
    // the seed's columns are unknown to the schema job (no catalog), so how
    // the column is made is not claimed; its input still is
    assert_eq!(id.transform, Some(Transform::Unknown), "{id:?}");
    assert_eq!(
        id.inputs,
        vec![Hop {
            node: Some("model:raw_customers".to_string()),
            column: "id".to_string(),
            relation: None,
        }]
    );
    // the seed learns the column from the hop
    let seed = node(&site, "model:raw_customers");
    assert!(!column(seed, "id").produced);
    // one hop further down: customers.customer_id reads stg_customers'
    let customers = node(&site, "model:customers");
    let hop = &column(customers, "customer_id").inputs;
    assert!(
        hop.iter()
            .any(|h| h.node.as_deref() == Some("model:stg_customers") && h.column == "customer_id"),
        "{hop:?}"
    );
    let passed = column(customers, "customer_id");
    assert_eq!(passed.transform, Some(Transform::Passthrough), "{passed:?}");
    let first_order = column(customers, "first_order");
    assert_eq!(
        first_order.transform,
        Some(Transform::Derived),
        "{first_order:?}"
    );
}

#[test]
fn control_flow_without_compiled_sql_is_declined() {
    let site = site_of(&context());
    let orders = node(&site, "model:orders");
    assert_eq!(orders.lineage, LineageState::Declined);
    assert!(
        orders.note.as_deref().unwrap_or("").contains("dbt compile"),
        "{orders:?}"
    );
    assert!(orders.columns.iter().all(|c| c.inputs.is_empty()));
    assert!(site.counts.models_declined >= 1, "{:?}", site.counts);
    assert!(site.counts.columns_traced > 0, "{:?}", site.counts);
    assert_eq!(
        site.counts.columns,
        site.counts.columns_traced + site.counts.columns_undetermined
    );
}

#[test]
fn the_output_is_stable() {
    let ctx = context();
    let a = serde_json::to_string(&site_of(&ctx)).unwrap();
    let b = serde_json::to_string(&site_of(&ctx)).unwrap();
    assert_eq!(a, b);
    let site = site_of(&ctx);
    let mut sorted = site.nodes.clone();
    sorted.sort_by(|a, b| a.layer.cmp(&b.layer).then_with(|| a.id.cmp(&b.id)));
    assert_eq!(
        sorted.iter().map(|n| &n.id).collect::<Vec<_>>(),
        site.nodes.iter().map(|n| &n.id).collect::<Vec<_>>()
    );
}

#[test]
fn a_staging_model_snapshot() {
    let site = site_of(&context());
    let mut stg = node(&site, "model:stg_customers").clone();
    stg.uri = stg
        .uri
        .map(|_| "file:///<fixture>/models/staging/stg_customers.sql".to_string());
    insta::assert_json_snapshot!(stg);
}

#[test]
fn the_page_embeds_the_data_and_fetches_nothing() {
    let mut site = site_of(&context());
    site.project = "</script><b>x".to_string();
    let html = page_html(&site);
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("<script type=\"application/json\" id=\"cogitator-data\">"));
    assert!(html.contains("\\u003c/script\\u003e\\u003cb\\u003ex"));
    assert!(!html.contains("</script><b>"));
    assert!(html.contains("&lt;/script&gt;&lt;b&gt;x \u{2014} lineage"));
    assert!(!html.contains("{{"), "a placeholder is left");
    for scheme in ["http://", "https://"] {
        let found: Vec<&str> = html
            .match_indices(scheme)
            .map(|(i, _)| &html[i..i + 40.min(html.len() - i)])
            .collect();
        // the SVG namespace is a name, never fetched
        assert!(
            found
                .iter()
                .all(|f| f.starts_with("http://www.w3.org/2000/svg")),
            "{found:?}"
        );
    }
    let data_start = html.find("id=\"cogitator-data\">").unwrap() + "id=\"cogitator-data\">".len();
    let data_end = data_start + html[data_start..].find("</script>").unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&html[data_start..data_end]).unwrap();
    assert_eq!(parsed["format"], FORMAT);
    assert_eq!(parsed["nodes"].as_array().unwrap().len(), site.nodes.len());
}

#[test]
fn docs_blocks_are_resolved() {
    let ctx = context();
    let Some((name, doc)) = ctx.doc_detail_map.iter().next() else {
        return;
    };
    let text = format!("before {{{{ doc('{name}') }}}} after");
    assert_eq!(
        docs::resolve_docs(&ctx, &text),
        format!("before {} after", doc.content.trim())
    );
    assert_eq!(
        docs::resolve_docs(&ctx, "{{ doc('nope') }}"),
        "{{ doc('nope') }}"
    );
    assert_eq!(docs::resolve_docs(&ctx, "plain {{"), "plain {{");
}
