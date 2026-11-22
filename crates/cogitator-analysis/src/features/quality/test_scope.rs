//! Which dbt node each test entry of a properties YAML becomes a test
//! on. Mostly one: the model, seed, snapshot or source table it is
//! written under. A versioned model is the exception (dbt-core's
//! `UnparsedModelUpdate.get_tests_for_version` and
//! `get_columns_for_version`): the base entry's own tests apply to every
//! version that lists no `data_tests:`/`tests:` of its own, and a base
//! column's tests to every version whose `include`/`exclude` keeps that
//! column, beside the version's own columns; nothing is named after the
//! base model itself.

use cogitator_schemas::yaml::{as_str, entries, get, items};
use saphyr::MarkedYaml;

use super::py_yaml::{py_str, Source};

/// One test entry applied to one dbt node.
pub struct Applied<'a, 'i> {
    pub item: &'a MarkedYaml<'i>,
    pub column: Option<&'a str>,
    /// The node name tests are named after: `orders`, `orders_v2`, or
    /// `<source>_<table>`.
    pub target: String,
    pub is_source: bool,
}

/// Every (test entry, node) pair of a properties YAML document.
pub fn applied_tests<'a, 'i>(root: &'a MarkedYaml<'i>, src: &Source<'_>) -> Vec<Applied<'a, 'i>> {
    let mut out = Vec::new();
    for key in ["models", "seeds", "snapshots"] {
        for entry in get(root, key).into_iter().flat_map(items) {
            let Some(name) = get(entry, "name").and_then(as_str) else {
                continue;
            };
            let versions: Vec<_> = get(entry, "versions").into_iter().flat_map(items).collect();
            if versions.is_empty() {
                node(
                    &mut out,
                    name.to_string(),
                    false,
                    tests_of(entry),
                    columns_of(entry),
                );
                continue;
            }
            for version in versions {
                let Some(v) = get(version, "v").map(|v| py_str(v, src)) else {
                    continue;
                };
                let own_tests = ["data_tests", "tests"]
                    .iter()
                    .any(|k| get(version, k).is_some());
                let tests = if own_tests {
                    tests_of(version)
                } else {
                    tests_of(entry)
                };
                let keep = IncludeExclude::of(version);
                let columns = columns_of(entry)
                    .into_iter()
                    .filter(|(name, _)| keep.includes(name))
                    .chain(columns_of(version))
                    .collect();
                node(&mut out, format!("{name}_v{v}"), false, tests, columns);
            }
        }
    }
    for source in get(root, "sources").into_iter().flat_map(items) {
        let Some(source_name) = get(source, "name").and_then(as_str) else {
            continue;
        };
        for table in get(source, "tables").into_iter().flat_map(items) {
            if let Some(table_name) = get(table, "name").and_then(as_str) {
                let target = format!("{source_name}_{table_name}");
                node(&mut out, target, true, tests_of(table), columns_of(table));
            }
        }
    }
    out
}

type Column<'a, 'i> = (&'a str, Vec<&'a MarkedYaml<'i>>);

fn node<'a, 'i>(
    out: &mut Vec<Applied<'a, 'i>>,
    target: String,
    is_source: bool,
    tests: Vec<&'a MarkedYaml<'i>>,
    columns: Vec<Column<'a, 'i>>,
) {
    let model_level = tests.into_iter().map(|item| (item, None));
    let column_level = columns
        .into_iter()
        .flat_map(|(name, tests)| tests.into_iter().map(move |item| (item, Some(name))));
    for (item, column) in model_level.chain(column_level) {
        out.push(Applied {
            item,
            column,
            target: target.clone(),
            is_source,
        });
    }
}

/// The items of a node's (or column's) `data_tests:` and legacy `tests:`.
fn tests_of<'a, 'i>(node: &'a MarkedYaml<'i>) -> Vec<&'a MarkedYaml<'i>> {
    ["data_tests", "tests"]
        .iter()
        .flat_map(|k| get(node, k).into_iter().flat_map(items))
        .collect()
}

/// A node's named columns with their tests (an `include`/`exclude`
/// element of a version's columns is no column).
fn columns_of<'a, 'i>(node: &'a MarkedYaml<'i>) -> Vec<Column<'a, 'i>> {
    get(node, "columns")
        .into_iter()
        .flat_map(items)
        .filter_map(|c| Some((get(c, "name").and_then(as_str)?, tests_of(c))))
        .collect()
}

/// dbt_common's `IncludeExclude` of a version's columns (default: all).
struct IncludeExclude<'a> {
    include: Option<Vec<&'a str>>,
    exclude: Vec<&'a str>,
}

impl<'a> IncludeExclude<'a> {
    fn of(version: &'a MarkedYaml<'_>) -> Self {
        let element = get(version, "columns")
            .into_iter()
            .flat_map(items)
            .find(|c| entries(c).any(|(k, _)| as_str(k) == Some("include")));
        let strings = |node: Option<&'a MarkedYaml<'_>>| -> Vec<&'a str> {
            node.into_iter()
                .flat_map(items)
                .filter_map(as_str)
                .collect()
        };
        match element {
            None => Self {
                include: None,
                exclude: Vec::new(),
            },
            Some(e) => {
                let include = get(e, "include");
                let all = include
                    .and_then(as_str)
                    .is_some_and(|s| s == "all" || s == "*");
                Self {
                    include: (!all).then(|| strings(include)),
                    exclude: strings(get(e, "exclude")),
                }
            }
        }
    }

    fn includes(&self, column: &str) -> bool {
        self.include.as_ref().is_none_or(|i| i.contains(&column)) && !self.exclude.contains(&column)
    }
}
