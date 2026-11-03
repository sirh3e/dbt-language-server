//! dbt.modelInfo: everything the :DbtBrowse preview needs for one model in
//! a single round trip — direct neighbors (the dbt.modelGraph entry shape),
//! versions and tests from the properties YAML (the dbt.modelTests shapes),
//! and the on-disk compiled SQL location when dbt has written one.

use std::path::{Path, PathBuf};

use cogitator_schemas::yaml::{get, items, to_json};
use saphyr::{LoadableYamlNode, MarkedYaml};
use serde::Serialize;

use super::graph_nav::{graph_entry, GraphEntry};
use super::tests_nav::{collect_model_tests, model_entry, resolve_model_name, ModelTest};
use super::{file_uri, model_name_from_uri, DbtContext};

/// The dbt.modelInfo result. `versions` and `compiled_uri` are `None` →
/// `null` on the wire (never omitted).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub model: String,
    pub uri: String,
    /// YAML description ("" when undocumented) and tags — for the
    /// explorer's info tab, so no extra round trips are needed.
    pub description: String,
    pub tags: Vec<String>,
    pub upstream: Vec<GraphEntry>,
    pub downstream: Vec<GraphEntry>,
    pub versions: Option<ModelVersions>,
    pub tests: Vec<ModelTest>,
    pub compiled_uri: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelVersions {
    pub latest_version: Option<i64>,
    pub versions: Vec<VersionEntry>,
}

#[derive(Debug, Serialize)]
pub struct VersionEntry {
    pub v: i64,
    pub uri: Option<String>,
}

/// `model_arg` (re-selecting in the browser) wins over `uri`; `None` when the
/// name resolves to nothing in the project.
pub fn model_info(ctx: &DbtContext, uri: &str, model_arg: Option<&str>) -> Option<ModelInfo> {
    let requested = match model_arg {
        Some(m) if !m.is_empty() => m.to_string(),
        _ => model_name_from_uri(uri)?,
    };
    // The exact graph node when its file exists (a `<base>_vN.sql` is its own
    // node); otherwise fall back to the base model, like dbt.modelTests.
    let name = if ctx.model_detail_map.contains_key(&requested) {
        requested
    } else {
        resolve_model_name(ctx, &requested)?
    };
    let details = ctx.model_detail_map.get(&name)?;
    let (upstream, downstream) = ctx.graph.model_neighbors(&name);

    let base = schema_base(ctx, &name);
    let (versions, tests) = ctx
        .model_detail_map
        .get(&base)
        .and_then(|d| d.schema_uri.as_ref())
        .and_then(|schema_path| schema_details(ctx, &base, schema_path))
        .unwrap_or((None, Vec::new()));

    let compiled_uri = ctx.project_root.as_deref().and_then(|root| {
        let compiled = compiled_sql_path(root, &ctx.project.name, &details.uri);
        compiled.is_file().then(|| file_uri(&compiled))
    });

    Some(ModelInfo {
        uri: file_uri(&details.uri),
        description: details.description.clone(),
        tags: details.tags.clone(),
        upstream: upstream.into_iter().map(|n| graph_entry(ctx, n)).collect(),
        downstream: downstream
            .into_iter()
            .map(|n| graph_entry(ctx, n))
            .collect(),
        versions,
        tests,
        compiled_uri,
        model: name,
    })
}

/// Where dbt writes the compiled model: `<root>/target/compiled/<project>/
/// <model path relative to root>` (pure path math; `target-path` overrides
/// are not honored anywhere in the server).
pub fn compiled_sql_path(root: &Path, project: &str, model_path: &Path) -> PathBuf {
    let relative = model_path.strip_prefix(root).unwrap_or(model_path);
    root.join("target")
        .join("compiled")
        .join(project)
        .join(relative)
}

/// The model whose schema entry documents `name`: itself when it has one,
/// else the `_vN`-stripped base (versioned files have no entry of their own).
fn schema_base(ctx: &DbtContext, name: &str) -> String {
    if ctx
        .model_detail_map
        .get(name)
        .is_some_and(|d| d.schema_uri.is_some())
    {
        return name.to_string();
    }
    name.rsplit_once("_v")
        .filter(|(base, v)| {
            v.chars().all(|c| c.is_ascii_digit()) && ctx.model_detail_map.contains_key(*base)
        })
        .map(|(base, _)| base.to_string())
        .unwrap_or_else(|| name.to_string())
}

/// Versions and tests from one read+parse of the base model's properties YAML.
fn schema_details(
    ctx: &DbtContext,
    model: &str,
    schema_path: &Path,
) -> Option<(Option<ModelVersions>, Vec<ModelTest>)> {
    let text = std::fs::read_to_string(schema_path).ok()?;
    let schema_uri = file_uri(schema_path);
    let docs = MarkedYaml::load_from_str(&text).ok()?;
    let root = docs.first()?;
    let entry = model_entry(root, model)?;
    Some((
        versions(ctx, model, entry),
        collect_model_tests(entry, &schema_uri),
    ))
}

/// The `versions:` block, or `None` for an unversioned model.
fn versions(ctx: &DbtContext, model: &str, entry: &MarkedYaml<'_>) -> Option<ModelVersions> {
    let list = get(entry, "versions")?;
    let latest_version = get(entry, "latest_version")
        .map(to_json)
        .and_then(|v| v.as_i64());
    let versions = items(list)
        .filter_map(|version| {
            let v = get(version, "v").map(to_json).and_then(|v| v.as_i64())?;
            Some(VersionEntry {
                uri: version_uri(ctx, model, v, latest_version),
                v,
            })
        })
        .collect();
    Some(ModelVersions {
        latest_version,
        versions,
    })
}

/// dbt's file convention: `<model>_v<N>.sql`, with the latest version allowed
/// to live in the unsuffixed `<model>.sql`.
fn version_uri(ctx: &DbtContext, model: &str, v: i64, latest: Option<i64>) -> Option<String> {
    let versioned = ctx
        .model_detail_map
        .get(&format!("{model}_v{v}"))
        .filter(|d| !d.uri.as_os_str().is_empty())
        .map(|d| file_uri(&d.uri));
    versioned.or_else(|| {
        (latest == Some(v))
            .then(|| ctx.model_detail_map.get(model))
            .flatten()
            .filter(|d| !d.uri.as_os_str().is_empty())
            .map(|d| file_uri(&d.uri))
    })
}
