//! dbt.impact (03 §10): the transitive downstream closure of a model,
//! with column granularity where per-model lineage resolves. Pure —
//! the shell fans out lineage computation and hands the map in.

use std::collections::HashMap;

use serde::Serialize;

use super::DbtContext;
use crate::graph::NodeKind;
use crate::lineage::{ColumnGraph, ModelLineage, Relation};

#[derive(Debug, Serialize)]
pub struct ImpactNode {
    pub name: String,
    pub uri: Option<String>,
    pub depth: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnHit {
    pub model: String,
    pub column: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImpactReport {
    pub model: String,
    pub column: Option<String>,
    /// "customer_id feeds 9 models, 2 exposures" — the editors' one-liner.
    pub summary: String,
    pub models: Vec<ImpactNode>,
    /// Exposures/metrics/semantic models in the closure.
    pub exposures: Vec<String>,
    pub column_hits: Vec<ColumnHit>,
    pub truncated: bool,
    /// Models whose column answer came from compiled SQL; omitted when
    /// none did (additive, like `ModelLineage.origin`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub compiled_lineage: Vec<String>,
    /// With a column: what it reaches past the models — tests, semantic
    /// models, metrics, exposures, dashboards (`features::column_reach`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reach: Option<super::column_reach::ColumnReach>,
}

impl ImpactReport {
    /// Attach a column reach; the one-liner then says what reads it.
    pub fn with_reach(mut self, reach: Option<super::column_reach::ColumnReach>) -> Self {
        if let Some(reach) = &reach {
            self.summary = format!("{}; {}", self.summary, reach.summary);
        }
        self.reach = reach;
        self
    }
}

/// How many downstream models the shell computes lineage for.
pub const IMPACT_LINEAGE_CAP: usize = 200;

/// The downstream models of `model`, depth-ordered — what the shell
/// needs lineage for before calling `impact`.
pub fn impact_targets(ctx: &DbtContext, model: &str) -> Vec<(String, std::path::PathBuf)> {
    ctx.graph
        .downstream_closure_with_depth(model)
        .into_iter()
        .filter(|(node, _)| node.kind == NodeKind::Model && !node.uri.as_os_str().is_empty())
        .take(IMPACT_LINEAGE_CAP)
        .map(|(node, _)| (node.name.clone(), node.uri.clone()))
        .collect()
}

/// The column graph dbt.impact walks: the downstream models' lineages.
/// Build it once to ask about several columns ([`impact_over`]).
pub fn impact_graph(lineages: &HashMap<String, ModelLineage>) -> ColumnGraph {
    let mut sorted: Vec<(&String, &ModelLineage)> = lineages.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0));
    ColumnGraph::build(sorted.into_iter().map(|(m, l)| (m.as_str(), l)))
}

pub fn impact(
    ctx: &DbtContext,
    model: &str,
    column: Option<&str>,
    lineages: &HashMap<String, ModelLineage>,
) -> ImpactReport {
    impact_over(ctx, model, column, lineages, &impact_graph(lineages))
}

/// [`impact`] over a graph [`impact_graph`] built of `lineages`.
pub fn impact_over(
    ctx: &DbtContext,
    model: &str,
    column: Option<&str>,
    lineages: &HashMap<String, ModelLineage>,
    graph: &ColumnGraph,
) -> ImpactReport {
    let closure = ctx.graph.downstream_closure_with_depth(model);
    let mut models = Vec::new();
    let mut exposures = Vec::new();
    for (node, depth) in &closure {
        match node.kind {
            NodeKind::Model => models.push(ImpactNode {
                name: node.name.clone(),
                uri: (!node.uri.as_os_str().is_empty())
                    .then(|| cogitator_lsp::fs_path_to_uri(&node.uri)),
                depth: *depth,
            }),
            NodeKind::Exposure | NodeKind::Metric | NodeKind::SemanticModel => {
                exposures.push(node.name.clone())
            }
            _ => {}
        }
    }
    let truncated = models.len() > IMPACT_LINEAGE_CAP;
    let compiled_lineage = models
        .iter()
        .filter(|node| lineages.get(&node.name).is_some_and(|l| l.origin.is_some()))
        .map(|node| node.name.clone())
        .collect();
    let column_hits = column
        .map(|column| column_closure(model, column, &models, graph))
        .unwrap_or_default();
    let subject = column
        .map(|c| format!("{model}.{c}"))
        .unwrap_or_else(|| model.to_string());
    let summary = format!(
        "{subject} feeds {} model(s), {} exposure(s)",
        models.len(),
        exposures.len()
    );
    ImpactReport {
        model: model.to_string(),
        column: column.map(str::to_string),
        summary,
        models,
        exposures,
        column_hits,
        truncated,
        compiled_lineage,
        reach: None,
    }
}

/// The downstream columns `model.column` feeds, from the column graph of
/// the downstream lineages: what reads it at any remove, renamed or passed
/// on by a `*` (it keeps its name there). A model's unresolved `*` stays
/// out: the hits are an under-approximation on purpose.
///
/// The graph decides what is reached; the order is for reading: the
/// model's place in the closure, then the column's in the model's outputs.
/// (A walk in closure order missed a model that reads a sibling sorting
/// after it at the same depth.)
fn column_closure(
    model: &str,
    column: &str,
    downstream: &[ImpactNode],
    graph: &ColumnGraph,
) -> Vec<ColumnHit> {
    let Some(start) = graph.find_model_column(model, column) else {
        return Vec::new();
    };
    let place: HashMap<&str, usize> = downstream
        .iter()
        .enumerate()
        .map(|(i, node)| (node.name.as_str(), i))
        .collect();
    let mut hits: Vec<(usize, usize, ColumnHit)> = graph
        .downstream(&[start], None)
        .into_iter()
        .filter_map(|reached| {
            let node = graph.column(reached.node);
            let Relation::Model(hit_model) = &node.relation else {
                return None;
            };
            let model_place = *place.get(hit_model.as_str())?;
            let column_place = graph
                .outputs(hit_model)
                .iter()
                .position(|&n| n == reached.node)?;
            (node.column != "*").then(|| {
                let hit = ColumnHit {
                    model: hit_model.clone(),
                    column: node.column.clone(),
                };
                (model_place, column_place, hit)
            })
        })
        .collect();
    hits.sort_by_key(|(m, c, _)| (*m, *c));
    hits.into_iter().map(|(_, _, hit)| hit).collect()
}
