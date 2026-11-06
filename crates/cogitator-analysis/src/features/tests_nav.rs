//! dbt.modelTests / dbt.goToTests: every test defined for a model in its
//! properties YAML — model-level, per column, and inside `versions:`
//! entries (a `<model>_v<N>.sql` buffer resolves to its base model).

use cogitator_lsp::Range;
use cogitator_schemas::yaml::{as_str, entries, get, items, position_of};
use saphyr::{LoadableYamlNode, MarkedYaml};
use serde::Serialize;

use super::{file_uri, model_name_from_uri, DbtContext};

#[derive(Debug, Serialize)]
pub struct ModelTest {
    pub name: String,
    pub column: Option<String>,
    pub version: Option<i64>,
    /// True for a `unit_tests:` entry (dbt unit tests) — data tests
    /// otherwise.
    pub unit: bool,
    pub uri: String,
    pub range: Range,
}

/// Resolve a model name against the project: exact, or `<base>_v<N>` → base.
pub(crate) fn resolve_model_name(ctx: &DbtContext, name: &str) -> Option<String> {
    if ctx.model_detail_map.contains_key(name) {
        return Some(name.to_string());
    }
    let (base, v) = name.rsplit_once("_v")?;
    if v.chars().all(|c| c.is_ascii_digit()) && ctx.model_detail_map.contains_key(base) {
        return Some(base.to_string());
    }
    None
}

/// Resolve the buffer's model name: exact, or `<base>_v<N>` → base.
fn resolve_model(ctx: &DbtContext, uri: &str) -> Option<String> {
    resolve_model_name(ctx, &model_name_from_uri(uri)?)
}

/// The `models:` entry named `model` in a parsed properties document.
pub(crate) fn model_entry<'a, 'b>(
    root: &'a MarkedYaml<'b>,
    model: &str,
) -> Option<&'a MarkedYaml<'b>> {
    items(get(root, "models")?).find(|m| get(m, "name").and_then(as_str) == Some(model))
}

/// Every test on a model entry: model-level, per column, and per `versions:` item.
pub(crate) fn collect_model_tests(entry: &MarkedYaml<'_>, schema_uri: &str) -> Vec<ModelTest> {
    let mut tests = Vec::new();
    collect_node_tests(entry, None, None, schema_uri, &mut tests);
    for version in get(entry, "versions").into_iter().flat_map(items) {
        let v = get(version, "v")
            .map(cogitator_schemas::yaml::to_json)
            .and_then(|v| v.as_i64());
        collect_node_tests(version, None, v, schema_uri, &mut tests);
    }
    tests
}

pub fn model_tests(ctx: &DbtContext, uri: &str) -> Option<(String, Vec<ModelTest>)> {
    let model = resolve_model(ctx, uri)?;
    let details = ctx.model_detail_map.get(&model)?;
    let schema_path = details.schema_uri.as_ref()?;
    let text = std::fs::read_to_string(schema_path).ok()?;
    let schema_uri = file_uri(schema_path);

    let docs = MarkedYaml::load_from_str(&text).ok()?;
    let root = docs.first()?;
    let entry = model_entry(root, &model)?;
    let mut tests = collect_model_tests(entry, &schema_uri);
    tests.extend(collect_unit_tests(root, &model, &schema_uri));
    Some((model, tests))
}

/// Tests directly on `node` plus on each of its `columns:` entries.
fn collect_node_tests(
    node: &MarkedYaml<'_>,
    column: Option<&str>,
    version: Option<i64>,
    uri: &str,
    out: &mut Vec<ModelTest>,
) {
    for key in ["data_tests", "tests"] {
        for item in get(node, key).into_iter().flat_map(items) {
            let name = match as_str(item) {
                Some(s) => s.to_string(),
                // a configured test: `- relationships: {...}` — the key names it
                None => match entries(item).next().and_then(|(k, _)| as_str(k)) {
                    Some(k) => k.to_string(),
                    None => continue,
                },
            };
            out.push(ModelTest {
                name,
                column: column.map(str::to_string),
                version,
                unit: false,
                uri: uri.to_string(),
                range: Range::zero_width(position_of(item)),
            });
        }
    }
    if column.is_none() {
        for col in get(node, "columns").into_iter().flat_map(items) {
            if let Some(col_name) = get(col, "name").and_then(as_str) {
                collect_node_tests(col, Some(col_name), version, uri, out);
            }
        }
    }
}

/// Top-level `unit_tests:` entries targeting `model` in the same
/// properties file (dbt unit tests live beside the model's entry by
/// convention; other files are out of scope by design).
pub(crate) fn collect_unit_tests(
    root: &MarkedYaml<'_>,
    model: &str,
    schema_uri: &str,
) -> Vec<ModelTest> {
    get(root, "unit_tests")
        .into_iter()
        .flat_map(items)
        .filter(|entry| get(entry, "model").and_then(as_str) == Some(model))
        .filter_map(|entry| {
            let name = get(entry, "name").and_then(as_str)?;
            Some(ModelTest {
                name: name.to_string(),
                column: None,
                version: None,
                unit: true,
                uri: schema_uri.to_string(),
                range: Range::zero_width(position_of(get(entry, "name")?)),
            })
        })
        .collect()
}

/// The `dbt.modelTests` wire shape.
#[derive(Debug, Serialize)]
pub struct ModelTests {
    pub model: String,
    pub tests: Vec<ModelTest>,
}

pub fn model_tests_json(ctx: &DbtContext, uri: &str) -> Option<ModelTests> {
    let (model, tests) = model_tests(ctx, uri)?;
    Some(ModelTests { model, tests })
}
