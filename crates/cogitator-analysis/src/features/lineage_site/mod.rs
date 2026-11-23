//! The lineage site (`cogitator lineage-site`, `dbt.lineageSite`): the
//! project DAG of `dbt.projectGraph`, every node's details and, for the
//! models whose lineage could be traced, each output column's inputs one
//! hop up — as one serializable value, and as a self-contained HTML page.
//!
//! Pure: the shell builds the context and the selector universe, runs the
//! schema job, and answers each model's lineage through a closure (it is
//! the one that reads model files and compiled SQL).

use std::collections::HashMap;

use crate::context::ModelKind;
use crate::features::plan::schemas::SchemaSnapshot;
use crate::graph::NodeKind;
use crate::selectors::{ResourceType, Universe};
use crate::DbtContext;

mod columns;
mod docs;
mod page;
mod referenced;
mod trace;
mod types;

pub use page::{page_html, page_script, page_style};
pub use trace::{trace_model, ModelText};
pub use types::*;

/// What [`export`] reads.
pub struct SiteInput<'a> {
    pub ctx: &'a DbtContext,
    pub universe: &'a Universe,
    /// The project schema job's snapshot: column types and how each
    /// column is made. `None` leaves both unknown.
    pub schemas: Option<&'a SchemaSnapshot>,
    /// A model's lineage, by name; asked once per SQL model.
    pub traced: &'a dyn Fn(&str) -> Traced,
    /// `cogitator <version>`.
    pub generator: &'a str,
}

/// The site's data. Nodes are sorted by layer, then id; edges by their
/// ends; so the same project gives the same bytes.
pub fn export(input: &SiteInput<'_>) -> LineageSite {
    let ctx = input.ctx;
    let (dag_nodes, dag_edges) = ctx.graph.dag();
    let tests = tests_by_node(input.universe);
    let mut nodes: Vec<SiteNode> = dag_nodes
        .iter()
        .map(|(data, layer)| {
            let mut node = columns::node_of(input, data.kind, &data.name, &data.uri);
            node.layer = *layer;
            if let Some(found) = tests.get(&node.id) {
                node.tests = found.clone();
            }
            node
        })
        .collect();
    referenced::add_referenced(&mut nodes);
    nodes.sort_by(|a, b| a.layer.cmp(&b.layer).then_with(|| a.id.cmp(&b.id)));
    let mut edges: Vec<SiteEdge> = dag_edges
        .iter()
        .map(|((fk, fname), (tk, tname))| SiteEdge {
            parent: node_id(*fk, fname),
            child: node_id(*tk, tname),
        })
        .collect();
    edges.sort_by(|a, b| (&a.parent, &a.child).cmp(&(&b.parent, &b.child)));
    edges.dedup();
    let counts = count(&nodes, edges.len());
    LineageSite {
        format: FORMAT,
        project: ctx.project.name.clone(),
        generator: input.generator.to_string(),
        nodes,
        edges,
        counts,
    }
}

/// The `dbt.projectGraph` id of a node.
pub(crate) fn node_id(kind: NodeKind, name: &str) -> String {
    format!("{}:{name}", kind.as_str())
}

/// The site kind of a graph node: seeds and snapshots are model nodes in
/// the graph and told apart by their details.
fn site_kind(ctx: &DbtContext, kind: NodeKind, name: &str) -> SiteKind {
    match kind {
        NodeKind::Model | NodeKind::Macro => match ctx.model_detail_map.get(name).map(|d| d.kind) {
            Some(ModelKind::Seed) => SiteKind::Seed,
            Some(ModelKind::Snapshot) => SiteKind::Snapshot,
            _ => SiteKind::Model,
        },
        NodeKind::Source => SiteKind::Source,
        NodeKind::Exposure => SiteKind::Exposure,
        NodeKind::Metric => SiteKind::Metric,
        NodeKind::SemanticModel => SiteKind::SemanticModel,
        NodeKind::Test => SiteKind::Test,
        NodeKind::Analysis => SiteKind::Analysis,
    }
}

/// The universe's tests, by the graph id of every node they test.
fn tests_by_node(u: &Universe) -> HashMap<String, Vec<String>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for (index, node) in u.nodes.iter().enumerate() {
        if node.resource_type != ResourceType::Test {
            continue;
        }
        for &parent in &u.parents[index] {
            let parent = &u.nodes[parent];
            let id = match (&parent.source, parent.resource_type) {
                (Some((source, _)), _) => node_id(NodeKind::Source, source),
                (None, ResourceType::Model | ResourceType::Seed | ResourceType::Snapshot) => {
                    node_id(NodeKind::Model, &parent.name)
                }
                _ => continue,
            };
            out.entry(id).or_default().push(node.name.clone()); // boundary: the site owns names
        }
    }
    for names in out.values_mut() {
        names.sort();
        names.dedup();
    }
    out
}

fn count(nodes: &[SiteNode], edges: usize) -> Counts {
    let mut c = Counts {
        nodes: nodes.len(),
        edges,
        ..Counts::default()
    };
    for node in nodes {
        match node.kind {
            SiteKind::Model => c.models += 1,
            SiteKind::Source => c.sources += 1,
            SiteKind::Seed => c.seeds += 1,
            SiteKind::Snapshot => c.snapshots += 1,
            SiteKind::Exposure => c.exposures += 1,
            _ => {}
        }
        match node.lineage {
            LineageState::Traced | LineageState::Compiled => {
                c.models_traced += 1;
                for column in node.columns.iter().filter(|col| col.produced) {
                    c.columns += 1;
                    if columns::is_traced(column) {
                        c.columns_traced += 1;
                    } else {
                        c.columns_undetermined += 1;
                    }
                }
            }
            LineageState::Declined => {
                c.models_declined += 1;
                let documented = node.columns.iter().filter(|col| col.documented).count();
                c.columns += documented;
                c.columns_undetermined += documented;
            }
            LineageState::NotApplicable => {}
        }
    }
    c
}

#[cfg(test)]
mod tests;
