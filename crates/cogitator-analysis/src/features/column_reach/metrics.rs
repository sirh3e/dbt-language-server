//! Metrics over measures and other metrics, read from the `metrics:`
//! section of properties YAML: simple and cumulative metrics name a
//! `measure`, conversion metrics a `base_measure`/`conversion_measure`,
//! ratio metrics a `numerator`/`denominator` metric, derived metrics a
//! list of `metrics`. Each may be a string or a mapping with `name:`.
//! A metric's `filter:` (Jinja `Dimension('…')`) is not followed.

use std::path::{Path, PathBuf};

use cogitator_lsp::Range;
use cogitator_schemas::yaml::{as_str, entries, get, items, range_of};
use saphyr::{LoadableYamlNode, MarkedYaml, YamlData};
use serde::Serialize;

/// One metric definition: what it is built from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MetricDef {
    pub name: String,
    pub path: PathBuf,
    pub range: Range,
    pub measures: Vec<String>,
    pub metrics: Vec<String>,
}

/// A metric that (transitively) reads the column.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricRead {
    pub name: String,
    pub uri: String,
    pub range: Range,
    /// `measure:<name>` or `metric:<name>`: what of it reads the column.
    pub via: String,
}

const MEASURE_KEYS: [&str; 3] = ["measure", "base_measure", "conversion_measure"];
const METRIC_KEYS: [&str; 2] = ["numerator", "denominator"];

pub(crate) fn metric_defs(text: &str, path: &Path) -> Vec<MetricDef> {
    if !text.contains("metrics:") {
        return Vec::new();
    }
    let Ok(docs) = MarkedYaml::load_from_str(text) else {
        return Vec::new();
    };
    let Some(section) = docs.first().and_then(|root| get(root, "metrics")) else {
        return Vec::new();
    };
    items(section)
        .filter_map(|item| {
            let name_node = get(item, "name")?;
            let mut def = MetricDef {
                name: as_str(name_node)?.to_string(),
                path: path.to_path_buf(),
                range: range_of(name_node),
                measures: Vec::new(),
                metrics: Vec::new(),
            };
            if let Some(params) = get(item, "type_params") {
                inputs(params, &mut def);
            }
            Some(def)
        })
        .collect()
}

/// A name: the string, or a mapping's `name:`.
fn named(node: &MarkedYaml<'_>) -> Option<String> {
    as_str(node)
        .or_else(|| get(node, "name").and_then(as_str))
        .map(str::to_string)
}

fn inputs(node: &MarkedYaml<'_>, def: &mut MetricDef) {
    if !matches!(node.data, YamlData::Mapping(_)) {
        return;
    }
    for (key, value) in entries(node) {
        let Some(key) = as_str(key) else { continue };
        if MEASURE_KEYS.contains(&key) {
            def.measures.extend(named(value));
        } else if METRIC_KEYS.contains(&key) {
            def.metrics.extend(named(value));
        } else if key == "metrics" {
            def.metrics.extend(items(value).filter_map(named));
        } else {
            inputs(value, def); // e.g. conversion_type_params
        }
    }
}

/// Every metric reading one of `measures`, then every metric reading
/// those, to a fixpoint; each once, with what reached it.
pub(crate) fn follow(defs: &[MetricDef], measures: &[String]) -> Vec<MetricRead> {
    let eq = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
    let mut out: Vec<MetricRead> = Vec::new();
    let read = |def: &MetricDef, via: String| MetricRead {
        name: def.name.clone(),
        uri: cogitator_lsp::fs_path_to_uri(&def.path),
        range: def.range,
        via,
    };
    for def in defs {
        if let Some(m) = def
            .measures
            .iter()
            .find(|m| measures.iter().any(|x| eq(x, m)))
        {
            out.push(read(def, format!("measure:{m}")));
        }
    }
    loop {
        let mut grew = false;
        for def in defs {
            if out.iter().any(|r| r.name == def.name) {
                continue;
            }
            let hit = def
                .metrics
                .iter()
                .find(|m| out.iter().any(|r| eq(&r.name, m)))
                .cloned();
            if let Some(m) = hit {
                out.push(read(def, format!("metric:{m}")));
                grew = true;
            }
        }
        if !grew {
            return out;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const YAML: &str = "metrics:
  - name: revenue
    type: simple
    type_params:
      measure: total_amount
  - name: orders_count
    type: simple
    type_params:
      measure:
        name: order_count
  - name: revenue_per_order
    type: ratio
    type_params:
      numerator: revenue
      denominator: {name: orders_count}
  - name: growth
    type: derived
    type_params:
      expr: a * 2
      metrics:
        - name: revenue_per_order
          alias: a
  - name: converted
    type: conversion
    type_params:
      conversion_type_params:
        base_measure: visits
        conversion_measure: {name: total_amount}
  - name: unrelated
    type: simple
    type_params:
      measure: other
";

    #[test]
    fn measures_reach_metrics_transitively() {
        let defs = metric_defs(YAML, Path::new("/p/metrics.yml"));
        assert_eq!(defs.len(), 6);
        assert_eq!(defs[4].measures, vec!["visits", "total_amount"]);
        let got: Vec<(String, String)> = follow(&defs, &["total_amount".to_string()])
            .into_iter()
            .map(|r| (r.name, r.via))
            .collect();
        let want = [
            ("revenue", "measure:total_amount"),
            ("converted", "measure:total_amount"),
            ("revenue_per_order", "metric:revenue"),
            ("growth", "metric:revenue_per_order"),
        ];
        assert_eq!(
            got,
            want.map(|(a, b)| (a.to_string(), b.to_string())).to_vec()
        );
        assert!(follow(&defs, &[]).is_empty());
        assert!(metric_defs("models: []\n", Path::new("x")).is_empty());
    }
}
