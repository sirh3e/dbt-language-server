//! The shapes a column reach reports, and the two lookups that need no
//! YAML walk: column tests and the BI manifest's dashboards.

use std::path::Path;

use cogitator_lsp::Range;
use serde::Serialize;

use super::super::rename_column::semantic::{SemanticItem, SemanticList, SemanticSite};
use crate::artifacts::bi_manifest::BiManifest;
use crate::features::DbtContext;

/// How precisely a consumer is known to read the column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Level {
    /// It names the column (or a column fed by it).
    Column,
    /// It names only the model: it may or may not read the column.
    Node,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestRead {
    pub model: String,
    pub column: String,
    pub test: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticRead {
    pub semantic_model: String,
    /// The model it is over, and the tainted column it reads.
    pub model: String,
    pub column: String,
    /// `entity` | `dimension` | `measure`; absent for an `expr:` that is
    /// no list item's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<SemanticList>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub uri: String,
    /// The word of the `expr:` (or the `name:`) that reads the column.
    pub range: Range,
}

impl SemanticRead {
    pub(super) fn of(item: SemanticItem, path: &Path, column: &str) -> Self {
        let range = match item.site {
            SemanticSite::Word(range) => range,
            SemanticSite::AddExpr { name, .. } => name,
        };
        SemanticRead {
            semantic_model: item.semantic_model,
            model: item.model,
            column: column.to_string(),
            list: item.list,
            name: item.name,
            uri: cogitator_lsp::fs_path_to_uri(path),
            range,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExposureRead {
    pub name: String,
    /// Always `node`: an exposure lists nodes, never columns.
    pub level: Level,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardHit {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    pub level: Level,
    /// What of the lineage it reads: the model, and the column when the
    /// manifest names one.
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<String>,
}

/// The column tests on `model.column` from its properties YAML.
pub fn column_tests(ctx: &DbtContext, model: &str, column: &str) -> Vec<String> {
    ctx.model_detail_map
        .get(model)
        .and_then(|d| {
            d.columns
                .iter()
                .find(|c| c.name.eq_ignore_ascii_case(column))
        })
        .map(|c| c.tests.iter().map(|t| t.name.clone()).collect())
        .unwrap_or_default()
}

/// Each dashboard once: column level when one of its reads names a
/// tainted column, else node level when it reads a model of the
/// closure as a whole. A read naming another column of a closure model
/// is no hit.
pub(super) fn dashboards(
    bi: &BiManifest,
    tainted: &[(&str, &str)],
    closure: &[&str],
) -> Vec<DashboardHit> {
    let eq = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
    let mut out = Vec::new();
    for dash in &bi.dashboards {
        let column_hit = dash.reads.iter().find(|r| {
            r.column
                .as_deref()
                .is_some_and(|c| tainted.iter().any(|(m, col)| eq(m, &r.model) && eq(col, c)))
        });
        let node_hit = || {
            dash.reads
                .iter()
                .find(|r| r.column.is_none() && closure.iter().any(|m| eq(m, &r.model)))
        };
        let (read, level) = match column_hit {
            Some(r) => (r, Level::Column),
            None => match node_hit() {
                Some(r) => (r, Level::Node),
                None => continue,
            },
        };
        out.push(DashboardHit {
            name: dash.name.clone(),
            url: dash.url.clone(),
            level,
            model: read.model.clone(),
            column: read.column.clone(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifacts::bi_manifest::parse;

    #[test]
    fn dashboard_levels() {
        let bi = parse(
            r#"{"dashboards":[
              {"name":"a","reads":[{"model":"orders","column":"amount"}]},
              {"name":"b","reads":[{"model":"orders","column":"status"}]},
              {"name":"c","reads":[{"model":"fct","column":"total"},{"model":"orders"}]},
              {"name":"d","reads":[{"model":"orders"}]},
              {"name":"e","reads":[{"model":"elsewhere"}]}
            ]}"#,
            Path::new("bi.json"),
        )
        .unwrap();
        let hits = dashboards(
            &bi,
            &[("orders", "amount"), ("fct", "total")],
            &["fct", "orders"],
        );
        let got: Vec<(&str, Level)> = hits.iter().map(|h| (h.name.as_str(), h.level)).collect();
        assert_eq!(
            got,
            vec![
                ("a", Level::Column),
                ("c", Level::Column),
                ("d", Level::Node)
            ]
        );
    }
}
