//! Every node a selection can pick, as dbt's manifest has them: models,
//! seeds, snapshots, source tables, singular, generic and unit tests,
//! analyses, exposures, metrics and semantic models, each with what the
//! selection methods read and its parents. Built once per context (the
//! server caches it): it reads each model's file for its `config()` and
//! the source tables it reads, and each properties file for its tests.

use std::collections::HashMap;
use std::path::Path;

use cogitator_dialects::Dialect;
use cogitator_lsp::Range;
use cogitator_parser::{DocumentParser, JinjaSqlParser, TokenKind};
use serde::Serialize;

use super::folder_config;
use crate::artifacts::manifest::ManifestIndex;
use crate::context::exposures::RefTarget;
use crate::context::ModelKind;
use crate::features::quality::test_nodes::{yaml_test_nodes, TestParent};
use crate::graph::NodeKind;
use crate::DbtContext;

/// One selectable node. What goes on the wire is what a client lists and
/// opens; the rest is what the methods read.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    /// dbt's unique_id: `model.<package>.<name>`, `source.<package>.<source>.<table>`…
    pub unique_id: String,
    pub resource_type: &'static str,
    pub name: String,
    pub package: String,
    /// The file it is defined in, relative to its project, `/`-separated.
    pub path: String,
    pub tags: Vec<String>,
    /// The file to open, and where in it for a node of a YAML file.
    pub uri: Option<String>,
    pub range: Option<Range>,
    /// The node's id in dbt.projectGraph, where it has one.
    pub graph_id: Option<String>,
    #[serde(skip)]
    pub fqn: Vec<String>,
    #[serde(skip)]
    pub materialized: Option<String>,
    #[serde(skip)]
    pub access: Option<String>,
    #[serde(skip)]
    pub group: Option<String>,
    /// `singular`, `generic` or `unit` for a test.
    #[serde(skip)]
    pub test_type: Option<&'static str>,
    /// The generic test as written (`not_null`, `dbt_utils.accepted_range`).
    #[serde(skip)]
    pub test_name: Option<String>,
    /// A source table's source.
    #[serde(skip)]
    pub source: Option<String>,
    /// The properties file that documents it (dbt's `patch_path`).
    #[serde(skip)]
    pub patch_path: Option<String>,
    #[serde(skip)]
    pub parents: Vec<usize>,
}

#[derive(Debug, Default)]
pub struct Universe {
    pub nodes: Vec<Node>,
    pub children: Vec<Vec<usize>>,
}

impl Universe {
    pub fn parents(&self, i: usize) -> &[usize] {
        &self.nodes[i].parents
    }

    pub fn children(&self, i: usize) -> &[usize] {
        &self.children[i]
    }
}

/// What a node depends on, before indices are known.
enum Dep {
    Model(String),
    Source(String, String),
    SourceTarget(String),
}

struct Draft {
    node: Node,
    deps: Vec<Dep>,
}

/// Build the universe of `ctx`, with the manifest's configs where
/// target/ has them.
pub fn build(ctx: &DbtContext, manifest: Option<&ManifestIndex>) -> Universe {
    let Some(root) = ctx.project_root.as_deref() else {
        return Universe::default();
    };
    let mut drafts = Vec::new();
    model_like(ctx, root, manifest, &mut drafts);
    sources(ctx, root, &mut drafts);
    files_of_graph(ctx, root, &mut drafts);
    citizens(ctx, root, &mut drafts);
    yaml_tests(ctx, root, &mut drafts);
    link(drafts)
}

fn relative(root: &Path, file: &Path, install_path: &str) -> String {
    let rel = file.strip_prefix(root).unwrap_or(file);
    let parts: Vec<String> = rel
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let install = if install_path.is_empty() {
        "dbt_packages"
    } else {
        install_path
    };
    // a package's file: its path inside the package
    let parts = if parts.first().map(String::as_str) == Some(install) && parts.len() > 2 {
        parts[2..].to_vec()
    } else {
        parts
    };
    parts.join("/")
}

/// The folders between the resource path (`models/`) and the file.
fn folders(path: &str, resource_paths: &[String]) -> Vec<String> {
    let mut parts: Vec<&str> = path.split('/').collect();
    parts.pop();
    let skip = resource_paths
        .iter()
        .map(|p| p.trim_end_matches('/').split('/').count())
        .find(|&n| {
            let prefix = parts.iter().take(n).copied().collect::<Vec<_>>().join("/");
            resource_paths
                .iter()
                .any(|p| p.trim_end_matches('/') == prefix)
        })
        .unwrap_or(1.min(parts.len()));
    parts.into_iter().skip(skip).map(str::to_string).collect()
}

fn uri(path: &Path) -> Option<String> {
    (!path.as_os_str().is_empty()).then(|| cogitator_lsp::fs_path_to_uri(path))
}

fn model_like(
    ctx: &DbtContext,
    root: &Path,
    manifest: Option<&ManifestIndex>,
    out: &mut Vec<Draft>,
) {
    let p = &ctx.project;
    for (name, d) in &ctx.model_detail_map {
        let (resource_type, paths, tree_key, default) = match d.kind {
            ModelKind::Model => ("model", &p.model_paths, "models", "view"),
            ModelKind::Seed => ("seed", &p.seed_paths, "seeds", "seed"),
            ModelKind::Snapshot => ("snapshot", &p.snapshot_paths, "snapshots", "snapshot"),
        };
        let package = if d.project_name.is_empty() {
            p.name.clone()
        } else {
            d.project_name.clone()
        };
        let path = relative(root, &d.uri, &p.packages_install_path);
        let dirs = folders(&path, paths);
        let dir_refs: Vec<&str> = dirs.iter().map(String::as_str).collect();
        let folder = folder_config::resolve(p.resource_configs.get(tree_key), &package, &dir_refs);
        let text = std::fs::read_to_string(&d.uri).unwrap_or_default();
        let mut tags = d.tags.clone();
        let own = if resource_type == "seed" {
            Vec::new()
        } else {
            crate::util::config_tags(&text)
        };
        for tag in own.into_iter().chain(folder.tags) {
            if !tags.contains(&tag) {
                tags.push(tag);
            }
        }
        let materialized = manifest
            .and_then(|m| m.models.get(name))
            .and_then(|n| n.config.materialized.clone())
            .or_else(|| {
                (resource_type == "model")
                    .then(|| crate::features::config_materialized(&text))
                    .flatten()
            })
            .or(folder.materialized)
            .or_else(|| Some(default.to_string()));
        let mut fqn = vec![package.clone()];
        fqn.extend(dirs);
        fqn.push(name.clone());
        let deps = if text.is_empty() {
            Vec::new()
        } else {
            dependencies(&text, &ctx.dialect)
        };
        out.push(Draft {
            node: Node {
                unique_id: format!("{resource_type}.{package}.{name}"),
                resource_type,
                name: name.clone(),
                package,
                path,
                tags,
                uri: uri(&d.uri),
                range: None,
                graph_id: Some(format!("model:{name}")),
                fqn,
                materialized,
                access: d.access.clone(),
                group: d.group.clone(),
                test_type: None,
                test_name: None,
                source: None,
                patch_path: d
                    .schema_uri
                    .as_deref()
                    .map(|yml| relative(root, yml, &p.packages_install_path)),
                parents: Vec::new(),
            },
            deps,
        });
    }
}

/// The models and source tables a SQL text reads.
fn dependencies(text: &str, dialect: &Dialect) -> Vec<Dep> {
    let Ok(doc) = JinjaSqlParser.parse(text, dialect) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut source: Option<String> = None;
    for token in &doc.tokens {
        match token.kind {
            TokenKind::Ref => out.push(Dep::Model(doc.literal(token).to_string())),
            TokenKind::SourceName => source = Some(doc.literal(token).to_string()),
            TokenKind::SourceTable => {
                if let Some(src) = source.take() {
                    out.push(Dep::Source(src, doc.literal(token).to_string()));
                }
            }
            _ => {}
        }
    }
    out
}

fn sources(ctx: &DbtContext, root: &Path, out: &mut Vec<Draft>) {
    let p = &ctx.project;
    for (source_name, src) in &ctx.source_detail_map {
        for (table, t) in &src.tables {
            let path = relative(root, &t.uri, &p.packages_install_path);
            let mut fqn = vec![p.name.clone()];
            fqn.extend(folders(&path, &p.model_paths));
            fqn.push(source_name.clone());
            fqn.push(table.clone());
            out.push(Draft {
                node: Node {
                    unique_id: format!("source.{}.{source_name}.{table}", p.name),
                    resource_type: "source",
                    name: table.clone(),
                    package: p.name.clone(),
                    path,
                    tags: Vec::new(),
                    uri: uri(&t.uri),
                    range: Some(t.range),
                    graph_id: Some(format!("source:{source_name}")),
                    fqn,
                    materialized: None,
                    access: None,
                    group: None,
                    test_type: None,
                    test_name: None,
                    source: Some(source_name.clone()),
                    patch_path: None,
                    parents: Vec::new(),
                },
                deps: Vec::new(),
            });
        }
    }
}

/// Singular tests and analyses: graph nodes with files of their own.
fn files_of_graph(ctx: &DbtContext, root: &Path, out: &mut Vec<Draft>) {
    let p = &ctx.project;
    for node in ctx.graph.nodes() {
        let (resource_type, paths) = match node.kind {
            NodeKind::Test => ("test", &p.test_paths),
            NodeKind::Analysis => ("analysis", &p.analysis_paths),
            _ => continue,
        };
        let path = relative(root, &node.uri, &p.packages_install_path);
        let mut fqn = vec![p.name.clone()];
        fqn.extend(folders(&path, paths));
        fqn.push(node.name.clone());
        let text = std::fs::read_to_string(&node.uri).unwrap_or_default();
        out.push(Draft {
            node: Node {
                unique_id: format!("{resource_type}.{}.{}", p.name, node.name),
                resource_type,
                name: node.name.clone(),
                package: p.name.clone(),
                path,
                tags: crate::util::config_tags(&text),
                uri: uri(&node.uri),
                range: None,
                graph_id: Some(format!("{}:{}", node.kind.as_str(), node.name)),
                fqn,
                materialized: (resource_type == "test").then(|| "test".to_string()),
                access: None,
                group: None,
                test_type: (resource_type == "test").then_some("singular"),
                test_name: None,
                source: None,
                patch_path: None,
                parents: Vec::new(),
            },
            deps: dependencies(&text, &ctx.dialect),
        });
    }
}

fn citizens(ctx: &DbtContext, root: &Path, out: &mut Vec<Draft>) {
    let p = &ctx.project;
    for (resource_type, map) in [
        ("exposure", &ctx.exposure_detail_map),
        ("metric", &ctx.metric_detail_map),
        ("semantic_model", &ctx.semantic_model_detail_map),
    ] {
        for (name, c) in map {
            let path = relative(root, &c.uri, &p.packages_install_path);
            let mut fqn = vec![p.name.clone()];
            fqn.extend(folders(&path, &p.model_paths));
            fqn.push(name.clone());
            let deps = c
                .depends
                .iter()
                .map(|d| match d {
                    RefTarget::Model(m) => Dep::Model(m.clone()),
                    RefTarget::Source(s, t) => Dep::Source(s.clone(), t.clone()),
                })
                .collect();
            out.push(Draft {
                node: Node {
                    unique_id: format!("{resource_type}.{}.{name}", p.name),
                    resource_type,
                    name: name.clone(),
                    package: p.name.clone(),
                    path,
                    tags: Vec::new(),
                    uri: uri(&c.uri),
                    range: Some(c.range),
                    graph_id: Some(format!("{resource_type}:{name}")),
                    fqn,
                    materialized: None,
                    access: None,
                    group: None,
                    test_type: None,
                    test_name: None,
                    source: None,
                    patch_path: None,
                    parents: Vec::new(),
                },
                deps,
            });
        }
    }
}

/// Generic and unit tests of the project's properties files.
fn yaml_tests(ctx: &DbtContext, root: &Path, out: &mut Vec<Draft>) {
    let p = &ctx.project;
    for file in crate::features::references::files::yaml_files(ctx) {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let path = relative(root, &file, &p.packages_install_path);
        for test in yaml_test_nodes(&text) {
            let resource_type = if test.unit { "unit_test" } else { "test" };
            let mut fqn = vec![p.name.clone()];
            fqn.extend(folders(&path, &p.model_paths));
            fqn.push(test.name.clone());
            let deps = test
                .parents
                .iter()
                .map(|parent| match parent {
                    TestParent::Node(m) => Dep::Model(strip_version(ctx, m)),
                    TestParent::Source(s, t) => Dep::Source(s.clone(), t.clone()),
                    TestParent::SourceTarget(target) => Dep::SourceTarget(target.clone()),
                })
                .collect();
            let first_parent = test.parents.first().and_then(|parent| match parent {
                TestParent::Node(m) => Some(strip_version(ctx, m)),
                _ => None,
            });
            out.push(Draft {
                node: Node {
                    unique_id: match (&first_parent, test.unit) {
                        (Some(model), true) => {
                            format!("unit_test.{}.{model}.{}", p.name, test.name)
                        }
                        _ => format!("test.{}.{}", p.name, test.name),
                    },
                    resource_type,
                    name: test.name.clone(),
                    package: p.name.clone(),
                    path: path.clone(),
                    tags: Vec::new(),
                    uri: uri(&file),
                    range: Some(test.range),
                    graph_id: None,
                    fqn,
                    materialized: (!test.unit).then(|| "test".to_string()),
                    access: None,
                    group: None,
                    test_type: Some(if test.unit { "unit" } else { "generic" }),
                    test_name: (!test.unit).then(|| test.test.clone()),
                    source: None,
                    patch_path: None,
                    parents: Vec::new(),
                },
                deps,
            });
        }
    }
}

/// `orders_v2` → `orders` where only the base is a node.
fn strip_version(ctx: &DbtContext, name: &str) -> String {
    crate::features::tests_nav::resolve_model_name(ctx, name).unwrap_or_else(|| name.to_string())
}

/// Indices for the dependencies, and the reverse edges.
fn link(drafts: Vec<Draft>) -> Universe {
    let mut by_model: HashMap<String, usize> = HashMap::new();
    let mut by_source: HashMap<(String, String), usize> = HashMap::new();
    for (i, d) in drafts.iter().enumerate() {
        match d.node.resource_type {
            "model" | "seed" | "snapshot" => {
                by_model.entry(d.node.name.clone()).or_insert(i);
            }
            "source" => {
                let source = d.node.source.clone().unwrap_or_default();
                by_source.insert((source, d.node.name.clone()), i);
            }
            _ => {}
        }
    }
    let mut nodes = Vec::with_capacity(drafts.len());
    for d in drafts {
        let mut node = d.node;
        for dep in d.deps {
            let at = match dep {
                Dep::Model(m) => by_model.get(&m).copied(),
                Dep::Source(s, t) => by_source.get(&(s, t)).copied(),
                Dep::SourceTarget(target) => by_source
                    .iter()
                    .find(|((s, t), _)| format!("{s}_{t}") == target)
                    .map(|(_, &i)| i),
            };
            if let Some(at) = at {
                if !node.parents.contains(&at) {
                    node.parents.push(at);
                }
            }
        }
        nodes.push(node);
    }
    let mut children = vec![Vec::new(); nodes.len()];
    for (i, node) in nodes.iter().enumerate() {
        for &parent in &node.parents {
            children[parent].push(i);
        }
    }
    Universe { nodes, children }
}
