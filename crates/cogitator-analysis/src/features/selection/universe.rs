//! Every node a selection can pick, as dbt's manifest has them: models
//! (a node per version of a versioned one), seeds, snapshots (from
//! `{% snapshot %}` blocks and from YAML), source tables, singular,
//! generic and unit tests, analyses, exposures, metrics and semantic
//! models, each with what the selection methods read and its parents.
//! Built once per context (the server caches it): it reads each model's
//! file for its `config()` and what it refs, and each properties file
//! for its tests, versions and snapshots.

use std::collections::HashMap;
use std::path::Path;

use cogitator_lsp::Range;
use serde::Serialize;

use crate::artifacts::manifest::ManifestIndex;
use crate::DbtContext;

/// One selectable node. What goes on the wire is what a client lists and
/// opens; the rest is what the methods read.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    /// dbt's unique_id: `model.<package>.<name>[.v<N>]`, `source.<package>.<source>.<table>`…
    pub unique_id: String,
    pub resource_type: &'static str,
    pub name: String,
    /// A versioned model's version (`1` of `orders.v1`).
    pub version: Option<String>,
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

impl Node {
    /// A node with what every node has; the builders fill in the rest.
    pub(super) fn new(
        resource_type: &'static str,
        name: &str,
        package: &str,
        path: String,
    ) -> Node {
        Node {
            unique_id: format!("{resource_type}.{package}.{name}"),
            resource_type,
            name: name.to_string(),
            version: None,
            package: package.to_string(),
            path,
            tags: Vec::new(),
            uri: None,
            range: None,
            graph_id: None,
            fqn: Vec::new(),
            materialized: None,
            access: None,
            group: None,
            test_type: None,
            test_name: None,
            source: None,
            patch_path: None,
            parents: Vec::new(),
        }
    }

    /// The name `dbt ls --output name` prints: `orders.v1`, `raw.items`.
    pub fn listed_name(&self) -> String {
        match (&self.version, &self.source) {
            (Some(v), _) => format!("{}.v{v}", self.name),
            (None, Some(source)) => format!("{source}.{}", self.name),
            _ => self.name.clone(),
        }
    }
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
#[derive(Clone)]
pub(super) enum Dep {
    /// A ref: a model, seed or snapshot by name, or a version as
    /// `<model>_v<N>` (how a test of one names it).
    Model(String),
    Source(String, String),
    /// `<source>_<table>` as a source test's YAML names it.
    SourceTarget(String),
    /// A macro it calls: what that macro refs counts as its own.
    Macro(String),
}

pub(super) struct Draft {
    pub node: Node,
    pub deps: Vec<Dep>,
    /// Further names a ref may reach this node by (a version's file stem).
    pub aliases: Vec<String>,
}

/// Build the universe of `ctx`, with the manifest's configs where
/// target/ has them.
pub fn build(ctx: &DbtContext, manifest: Option<&ManifestIndex>) -> Universe {
    use super::{deps, projects, universe_nodes as nodes, yaml_nodes};
    let Some(root) = ctx.project_root.as_deref() else {
        return Universe::default();
    };
    let projects = projects::projects(ctx, root);
    let yaml = projects::read_yaml(&projects);
    let versions = yaml_nodes::versions(&yaml);
    let mut drafts = Vec::new();
    nodes::model_like(ctx, &projects, manifest, &versions, &mut drafts);
    yaml_nodes::yaml_snapshots(ctx, &projects, &yaml, &mut drafts);
    nodes::sources(ctx, root, &mut drafts);
    nodes::files_of_graph(ctx, &projects, &mut drafts);
    nodes::citizens(ctx, root, &mut drafts);
    yaml_nodes::yaml_tests(&projects, &yaml, &versions, &mut drafts);
    nodes::package_seeds(ctx, &projects, &mut drafts);
    let macros = deps::macros(ctx, &projects);
    for draft in &mut drafts {
        expand_macros(&mut draft.deps, &macros);
    }
    link(drafts)
}

/// `file` relative to its project: the root's, or a package's own
/// (`dbt_packages/<package>/models/x.sql` → `models/x.sql`).
pub(super) fn relative(root: &Path, file: &Path, install_path: &str) -> String {
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
    let parts = if parts.first().map(String::as_str) == Some(install) && parts.len() > 2 {
        parts[2..].to_vec()
    } else {
        parts
    };
    parts.join("/")
}

/// The folders between the resource path (`models/`) and the file.
pub(super) fn folders(path: &str, resource_paths: &[String]) -> Vec<String> {
    let mut parts: Vec<&str> = path.split('/').collect();
    parts.pop();
    let skip = resource_paths
        .iter()
        .map(|p| p.trim_end_matches('/'))
        .filter(|p| {
            let n = p.split('/').count();
            parts.iter().take(n).copied().collect::<Vec<_>>().join("/") == *p
        })
        .map(|p| p.split('/').count())
        .max()
        .unwrap_or(1.min(parts.len()));
    parts.into_iter().skip(skip).map(str::to_string).collect()
}

pub(super) fn uri(path: &Path) -> Option<String> {
    (!path.as_os_str().is_empty()).then(|| cogitator_lsp::fs_path_to_uri(path))
}

/// Replace the macros a node calls with what they ref, through the
/// macros they call in turn.
fn expand_macros(deps: &mut Vec<Dep>, macros: &HashMap<String, Vec<Dep>>) {
    let mut seen = std::collections::HashSet::new();
    let mut i = 0;
    while i < deps.len() {
        if let Dep::Macro(name) = &deps[i] {
            let name = name.clone();
            deps.swap_remove(i);
            if seen.insert(name.clone()) {
                if let Some(body) = macros.get(&name) {
                    deps.extend(body.iter().cloned());
                }
            }
            continue;
        }
        i += 1;
    }
}

/// Indices for the dependencies, and the reverse edges.
fn link(drafts: Vec<Draft>) -> Universe {
    let mut by_model: HashMap<String, usize> = HashMap::new();
    let mut by_source: HashMap<(String, String), usize> = HashMap::new();
    for (i, d) in drafts.iter().enumerate() {
        match d.node.resource_type {
            "model" | "seed" | "snapshot" => {
                for alias in &d.aliases {
                    by_model.insert(alias.clone(), i);
                }
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
                Dep::Model(m) => by_model.get(&m).copied().or_else(|| {
                    // `orders_v2` where the version is no file of its own
                    let (base, v) = m.rsplit_once("_v")?;
                    if v.chars().all(|c| c.is_ascii_digit()) {
                        by_model.get(base).copied()
                    } else {
                        None
                    }
                }),
                Dep::Source(s, t) => by_source.get(&(s, t)).copied(),
                Dep::SourceTarget(target) => by_source
                    .iter()
                    .find(|((s, t), _)| format!("{s}_{t}") == target)
                    .map(|(_, &i)| i),
                Dep::Macro(_) => None,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_are_below_the_resource_path() {
        let paths = vec!["models".to_string()];
        assert_eq!(
            folders("models/dim/inventory/x.sql", &paths),
            vec!["dim", "inventory"]
        );
        assert_eq!(folders("models/x.sql", &paths), Vec::<String>::new());
        assert_eq!(folders("other/a/x.sql", &paths), vec!["a"]);
        let nested = vec!["src/models".to_string()];
        assert_eq!(folders("src/models/dim/x.sql", &nested), vec!["dim"]);
    }
}
