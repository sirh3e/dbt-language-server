//! "Who reads this column?" (docs/column-reach.md): what a model's
//! column reaches past the models `dbt.impact`'s taint walk finds —
//! the column tests on every hit, the semantic-model entities,
//! dimensions and measures whose `expr:` (or name) reads a tainted
//! column, the metrics over those measures (transitively), the
//! exposures (node level only: an exposure names nodes, never columns)
//! and the dashboards of the BI manifest, the end of the lineage.
//! Pure: YAML text comes through the `read` closure.

mod metrics;
mod reads;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::rename_column::semantic::semantic_items;
use super::{DbtContext, ImpactReport};
use crate::artifacts::bi_manifest::BiManifest;
use metrics::{follow, metric_defs, MetricDef};

pub use metrics::MetricRead;
pub use reads::{column_tests, DashboardHit, ExposureRead, Level, SemanticRead, TestRead};

/// The column reach of one column, as `dbt.impact` reports it.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnReach {
    /// "read by 3 models, 2 tests, 1 semantic model, 1 dashboard".
    pub summary: String,
    /// Distinct downstream models with a column hit.
    pub models: usize,
    pub tests: Vec<TestRead>,
    pub semantic_items: Vec<SemanticRead>,
    /// Distinct semantic models among `semantic_items`.
    pub semantic_models: usize,
    pub metrics: Vec<MetricRead>,
    pub exposures: Vec<ExposureRead>,
    pub dashboards: Vec<DashboardHit>,
    /// Why a part is missing (an unreadable BI manifest, …).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

/// What every column of one model reaches: the hover's source.
#[derive(Debug, Clone, Default)]
pub struct ModelReach {
    pub model: String,
    /// Lowercased column name → its reach.
    pub columns: HashMap<String, ColumnReach>,
}

impl ModelReach {
    pub fn column(&self, name: &str) -> Option<&ColumnReach> {
        self.columns.get(&name.to_ascii_lowercase())
    }
}

/// The project's properties YAML, read once for several columns.
pub struct Corpus {
    yaml: Vec<(PathBuf, String)>,
    metrics: Vec<MetricDef>,
}

impl Corpus {
    pub fn read(ctx: &DbtContext, read: &dyn Fn(&Path) -> Option<String>) -> Self {
        let yaml: Vec<(PathBuf, String)> = super::references::files::yaml_files(ctx)
            .into_iter()
            .filter_map(|p| read(&p).map(|t| (p, t)))
            .collect();
        let metrics = yaml
            .iter()
            .flat_map(|(path, text)| metric_defs(text, path))
            .collect();
        Corpus { yaml, metrics }
    }
}

/// The reach of `report`'s column (`None` for a model-level report).
pub fn reach(
    ctx: &DbtContext,
    report: &ImpactReport,
    corpus: &Corpus,
    bi: Option<&BiManifest>,
) -> Option<ColumnReach> {
    let column = report.column.as_deref()?;
    let mut tainted: Vec<(&str, &str)> = vec![(report.model.as_str(), column)];
    tainted.extend(
        report
            .column_hits
            .iter()
            .map(|h| (h.model.as_str(), h.column.as_str())),
    );
    let mut out = ColumnReach {
        models: {
            let mut m: Vec<&str> = report
                .column_hits
                .iter()
                .map(|h| h.model.as_str())
                .collect();
            m.sort_unstable();
            m.dedup();
            m.len()
        },
        ..ColumnReach::default()
    };
    for (model, col) in &tainted {
        out.tests.extend(
            column_tests(ctx, model, col)
                .into_iter()
                .map(|test| TestRead {
                    model: model.to_string(),
                    column: col.to_string(),
                    test,
                }),
        );
        for (path, text) in &corpus.yaml {
            let over = |m: &str| m.eq_ignore_ascii_case(model);
            for item in semantic_items(text, &ctx.dialect, &over, col) {
                out.semantic_items.push(SemanticRead::of(item, path, col));
            }
        }
    }
    let mut names: Vec<&str> = out
        .semantic_items
        .iter()
        .map(|s| s.semantic_model.as_str())
        .collect();
    names.sort_unstable();
    names.dedup();
    out.semantic_models = names.len();
    let measures: Vec<String> = out
        .semantic_items
        .iter()
        .filter(|s| s.list == Some(super::rename_column::semantic::SemanticList::Measure))
        .filter_map(|s| s.name.clone())
        .collect();
    out.metrics = follow(&corpus.metrics, &measures);
    out.exposures = report
        .exposures
        .iter()
        .filter(|name| ctx.exposure_detail_map.contains_key(*name))
        .map(|name| ExposureRead {
            name: name.clone(),
            level: Level::Node,
        })
        .collect();
    if let Some(bi) = bi {
        let mut closure: Vec<&str> = report.models.iter().map(|m| m.name.as_str()).collect();
        closure.push(&report.model);
        out.dashboards = reads::dashboards(bi, &tainted, &closure);
    }
    out.summary = summary(&out);
    Some(out)
}

/// Every column of `model` at once, for the hover cache; `columns`
/// are the model's output columns, `lineages` dbt.impact's fan-out.
pub fn model_reach(
    ctx: &DbtContext,
    model: &str,
    columns: &[String],
    lineages: &HashMap<String, crate::lineage::ModelLineage>,
    corpus: &Corpus,
    bi: Option<&BiManifest>,
) -> ModelReach {
    let graph = super::impact_graph(lineages);
    let columns = columns
        .iter()
        .filter_map(|c| {
            let report = super::impact_over(ctx, model, Some(c), lineages, &graph);
            Some((c.to_ascii_lowercase(), reach(ctx, &report, corpus, bi)?))
        })
        .collect();
    ModelReach {
        model: model.to_string(),
        columns,
    }
}

/// The model and column the cursor names: a YAML column entry, or a
/// SQL word that is an output column (the column rename's rule).
pub fn column_at(
    ctx: &DbtContext,
    doc: &crate::Document,
    uri: &str,
    pos: cogitator_lsp::Position,
) -> Option<(String, String)> {
    super::rename_column::column_at(ctx, doc, uri, pos).map(|(m, c, _)| (m, c))
}

/// The hover line of a column's cached reach.
pub fn hover_line(model: &str, column: &str, reach: &ColumnReach) -> String {
    format!("`{model}.{column}` is {}", reach.summary)
}

fn plural(n: usize, one: &str) -> String {
    match n {
        1 => format!("1 {one}"),
        n => format!("{n} {one}s"),
    }
}

/// "read by 3 models, 2 tests, 1 semantic model, 0 dashboards", plus
/// metrics and node-level exposures when there are any.
pub fn summary(reach: &ColumnReach) -> String {
    let mut parts = vec![
        plural(reach.models, "model"),
        plural(reach.tests.len(), "test"),
        plural(reach.semantic_models, "semantic model"),
        plural(reach.dashboards.len(), "dashboard"),
    ];
    if !reach.metrics.is_empty() {
        parts.push(plural(reach.metrics.len(), "metric"));
    }
    if !reach.exposures.is_empty() {
        parts.push(format!(
            "{} (node level)",
            plural(reach.exposures.len(), "exposure")
        ));
    }
    format!("read by {}", parts.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summaries() {
        let mut reach = ColumnReach {
            models: 3,
            semantic_models: 1,
            ..ColumnReach::default()
        };
        assert_eq!(
            summary(&reach),
            "read by 3 models, 0 tests, 1 semantic model, 0 dashboards"
        );
        reach.exposures.push(ExposureRead {
            name: "weekly".into(),
            level: Level::Node,
        });
        assert!(summary(&reach).ends_with(", 1 exposure (node level)"));
    }
}
