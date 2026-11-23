//! The builders of the universe, one per kind of node, over the root
//! project and every installed package (see `projects`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::deps::dependencies;
use super::projects::{folder_of, path_in, Project};
use super::universe::{folders, relative, uri, Dep, Draft, Node};
use super::universe_yaml::ModelVersion;
use crate::artifacts::manifest::ManifestIndex;
use crate::context::exposures::RefTarget;
use crate::context::ModelKind;
use crate::graph::{defines_generic_test, is_fixture, NodeKind};
use crate::DbtContext;

pub(super) fn fqn(package: &str, dirs: Vec<String>, name: &str) -> Vec<String> {
    std::iter::once(package.to_string())
        .chain(dirs)
        .chain(std::iter::once(name.to_string()))
        .collect()
}

pub(super) fn model_like(
    ctx: &DbtContext,
    projects: &[Project],
    manifest: Option<&ManifestIndex>,
    versions: &HashMap<String, ModelVersion>,
    out: &mut Vec<Draft>,
) {
    let root = &projects[0];
    for (stem, d) in &ctx.model_detail_map {
        let package = if d.project_name.is_empty() {
            root.file.name.clone()
        } else {
            d.project_name.clone()
        };
        let project = projects
            .iter()
            .find(|p| p.file.name == package)
            .unwrap_or(root);
        let f = &project.file;
        let (resource_type, paths, tree_key, default) = match d.kind {
            ModelKind::Model => ("model", &f.model_paths, "models", "view"),
            ModelKind::Seed => ("seed", &f.seed_paths, "seeds", "seed"),
            ModelKind::Snapshot => ("snapshot", &f.snapshot_paths, "snapshots", "snapshot"),
        };
        let version = (resource_type == "model")
            .then(|| versions.get(stem))
            .flatten();
        let name = version.map_or(stem.as_str(), |v| v.model.as_str());
        let path = path_in(project, &d.uri);
        let dirs = folders(&path, paths);
        let folder = folder_of(projects, &package, tree_key, &dirs);
        let text = std::fs::read_to_string(&d.uri).unwrap_or_default();
        let mut node = Node::new(resource_type, name, &package, path);
        node.tags = d.tags.clone();
        let own = if resource_type == "seed" {
            Vec::new()
        } else {
            crate::util::config_tags(&text)
        };
        for tag in own.into_iter().chain(folder.tags) {
            if !node.tags.contains(&tag) {
                node.tags.push(tag);
            }
        }
        node.materialized = manifest
            .and_then(|m| m.models.get(stem).or_else(|| m.models.get(name)))
            .and_then(|n| n.config.materialized.clone())
            .or_else(|| {
                (resource_type == "model")
                    .then(|| crate::features::config_materialized(&text))
                    .flatten()
            })
            .or(folder.materialized)
            .or_else(|| Some(default.to_string()));
        node.fqn = fqn(&package, dirs, name);
        let mut aliases = vec![stem.clone()];
        if let Some(v) = version {
            node.version = Some(v.version.clone());
            node.unique_id = format!("model.{package}.{name}.v{}", v.version);
            node.fqn.push(format!("v{}", v.version));
            aliases.push(format!("{name}_v{}", v.version));
            if v.latest {
                // a plain ref('orders') reaches the latest version
                aliases.push(name.to_string());
            }
        }
        node.uri = uri(&d.uri);
        node.graph_id = Some(format!("model:{stem}"));
        node.access = d.access.clone();
        node.group = d.group.clone();
        node.patch_path = d.schema_uri.as_deref().map(|yml| path_in(project, yml));
        let deps = if text.is_empty() {
            Vec::new()
        } else {
            dependencies(&text, &ctx.dialect)
        };
        out.push(Draft {
            node,
            deps,
            aliases,
        });
    }
}

/// The packages' seeds (the context holds the root's alone).
pub(super) fn package_seeds(ctx: &DbtContext, projects: &[Project], out: &mut Vec<Draft>) {
    for project in projects.iter().filter(|p| !p.is_root) {
        let package = &project.file.name;
        for rel in &project.file.seed_paths {
            for file in crate::util::walk_filepath(&project.root.join(rel), ".csv") {
                let name = file
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if name.is_empty() || ctx.model_detail_map.contains_key(&name) {
                    continue;
                }
                let path = path_in(project, &file);
                let dirs = folders(&path, &project.file.seed_paths);
                let folder = folder_of(projects, package, "seeds", &dirs);
                let mut node = Node::new("seed", &name, package, path);
                node.fqn = fqn(package, dirs, &name);
                node.tags = folder.tags;
                node.materialized = Some("seed".to_string());
                node.uri = uri(&file);
                out.push(Draft {
                    node,
                    deps: Vec::new(),
                    aliases: Vec::new(),
                });
            }
        }
    }
}

pub(super) fn sources(ctx: &DbtContext, root: &Path, out: &mut Vec<Draft>) {
    let p = &ctx.project;
    for (source_name, src) in &ctx.source_detail_map {
        for (table, t) in &src.tables {
            let path = relative(root, &t.uri, &p.packages_install_path);
            let mut node = Node::new("source", table, &p.name, path.clone());
            node.unique_id = format!("source.{}.{source_name}.{table}", p.name);
            node.fqn = std::iter::once(p.name.clone())
                .chain(folders(&path, &p.model_paths))
                .chain([source_name.clone(), table.clone()])
                .collect();
            node.uri = uri(&t.uri);
            node.range = Some(t.range);
            node.graph_id = Some(format!("source:{source_name}"));
            node.source = Some(source_name.clone());
            out.push(Draft {
                node,
                deps: Vec::new(),
                aliases: Vec::new(),
            });
        }
    }
}

/// Singular tests and analyses: the root's from the graph, a package's
/// from its own test paths (the graph holds the root's alone).
pub(super) fn files_of_graph(ctx: &DbtContext, projects: &[Project], out: &mut Vec<Draft>) {
    let root = &projects[0];
    let mut files: Vec<(&Project, &'static str, &'static str, PathBuf, String)> = Vec::new();
    for g in ctx.graph.nodes() {
        let (resource_type, kind) = match g.kind {
            NodeKind::Test => ("test", "test"),
            NodeKind::Analysis => ("analysis", "analysis"),
            _ => continue,
        };
        files.push((root, resource_type, kind, g.uri.clone(), g.name.clone()));
    }
    for project in projects.iter().filter(|p| !p.is_root) {
        for rel in &project.file.test_paths {
            for file in crate::util::walk_filepath(&project.root.join(rel), ".sql") {
                if is_fixture(&project.root, &project.file.test_paths, &file)
                    || defines_generic_test(&file)
                {
                    continue;
                }
                let stem = file
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                files.push((project, "test", "test", file, stem));
            }
        }
    }
    for (project, resource_type, kind, file, name) in files {
        let path = path_in(project, &file);
        let paths = if resource_type == "test" {
            &project.file.test_paths
        } else {
            &project.file.analysis_paths
        };
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        let package = &project.file.name;
        let mut node = Node::new(resource_type, &name, package, path.clone());
        node.fqn = fqn(package, folders(&path, paths), &name);
        node.tags = crate::util::config_tags(&text);
        node.uri = uri(&file);
        if project.is_root {
            node.graph_id = Some(format!("{kind}:{name}"));
        }
        if resource_type == "test" {
            node.materialized = Some("test".to_string());
            node.test_type = Some("singular");
        }
        out.push(Draft {
            node,
            deps: dependencies(&text, &ctx.dialect),
            aliases: Vec::new(),
        });
    }
}

pub(super) fn citizens(ctx: &DbtContext, root: &Path, out: &mut Vec<Draft>) {
    let p = &ctx.project;
    for (resource_type, map) in [
        ("exposure", &ctx.exposure_detail_map),
        ("metric", &ctx.metric_detail_map),
        ("semantic_model", &ctx.semantic_model_detail_map),
    ] {
        for (name, c) in map {
            let path = relative(root, &c.uri, &p.packages_install_path);
            let mut node = Node::new(resource_type, name, &p.name, path.clone());
            node.fqn = fqn(&p.name, folders(&path, &p.model_paths), name);
            node.uri = uri(&c.uri);
            node.range = Some(c.range);
            node.graph_id = Some(format!("{resource_type}:{name}"));
            let deps = c
                .depends
                .iter()
                .map(|d| match d {
                    RefTarget::Model(m) => Dep::Model(m.clone()),
                    RefTarget::Source(s, t) => Dep::Source(s.clone(), t.clone()),
                })
                .collect();
            out.push(Draft {
                node,
                deps,
                aliases: Vec::new(),
            });
        }
    }
}
