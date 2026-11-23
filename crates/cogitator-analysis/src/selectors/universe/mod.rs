//! The nodes a selector chooses among, with what dbt's methods read of
//! them and the edges the graph operators walk. Built once per request
//! from the context plus a `read` closure (the shell's file access).

use std::path::PathBuf;

use cogitator_lsp::Range;

mod build;
pub mod config;
mod packages;
mod properties;
mod sql_deps;
mod tests_nodes;

/// dbt's `resource_type` of a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResourceType {
    Model,
    Seed,
    Snapshot,
    Test,
    UnitTest,
    Source,
    Exposure,
    Metric,
    SemanticModel,
    Analysis,
}

impl ResourceType {
    pub const ALL: [ResourceType; 10] = [
        ResourceType::Model,
        ResourceType::Seed,
        ResourceType::Snapshot,
        ResourceType::Test,
        ResourceType::UnitTest,
        ResourceType::Source,
        ResourceType::Exposure,
        ResourceType::Metric,
        ResourceType::SemanticModel,
        ResourceType::Analysis,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ResourceType::Model => "model",
            ResourceType::Seed => "seed",
            ResourceType::Snapshot => "snapshot",
            ResourceType::Test => "test",
            ResourceType::UnitTest => "unit_test",
            ResourceType::Source => "source",
            ResourceType::Exposure => "exposure",
            ResourceType::Metric => "metric",
            ResourceType::SemanticModel => "semantic_model",
            ResourceType::Analysis => "analysis",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.as_str() == name)
    }

    /// Nodes dbt keeps in `manifest.nodes`: what fqn, tag, config and
    /// the other "parsed node" methods look at, as opposed to sources,
    /// exposures, metrics and semantic models.
    pub fn is_parsed_node(self) -> bool {
        matches!(
            self,
            ResourceType::Model
                | ResourceType::Seed
                | ResourceType::Snapshot
                | ResourceType::Test
                | ResourceType::Analysis
        )
    }
}

/// `version:` of a versioned model, relative to its `latest_version`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionKind {
    Latest,
    Prerelease,
    Old,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TestInfo {
    /// `generic`, `singular` or `unit`.
    pub test_type: &'static str,
    /// The generic test's name (`unique`, `accepted_values`, a package
    /// test's macro name without the package).
    pub test_name: Option<String>,
}

/// One selectable node.
#[derive(Debug, Clone)]
pub struct Node {
    pub resource_type: ResourceType,
    pub name: String,
    pub package: String,
    /// The defining file relative to its package root, `/`-separated
    /// (dbt's `original_file_path`).
    pub path: String,
    pub uri: PathBuf,
    pub range: Range,
    pub fqn: Vec<String>,
    pub tags: Vec<String>,
    /// The resolved config (dbt_project.yml, properties `config:`, SQL).
    pub config: serde_json::Map<String, serde_json::Value>,
    pub group: Option<String>,
    pub access: Option<String>,
    pub version: Option<VersionKind>,
    /// A versioned model's `v`.
    pub version_number: Option<String>,
    pub test: Option<TestInfo>,
    /// A source table's (source, table).
    pub source: Option<(String, String)>,
}

impl Node {
    pub fn new(resource_type: ResourceType, name: &str, package: &str) -> Self {
        Node {
            resource_type,
            name: name.to_string(),
            package: package.to_string(),
            path: String::new(),
            uri: PathBuf::new(),
            range: Range::default(),
            fqn: vec![package.to_string(), name.to_string()],
            tags: Vec::new(),
            config: serde_json::Map::new(),
            group: None,
            access: None,
            version: None,
            version_number: None,
            test: None,
            source: None,
        }
    }

    /// The name a person reads: `source.table` for a source table.
    pub fn display_name(&self) -> String {
        match (&self.source, &self.version_number) {
            (Some((source, table)), _) => format!("{source}.{table}"),
            (None, Some(v)) => format!("{}.v{v}", self.name),
            (None, None) => self.name.clone(),
        }
    }
}

/// Every node plus its dependency edges, indexed by position.
#[derive(Debug, Clone, Default)]
pub struct Universe {
    pub nodes: Vec<Node>,
    /// The root project's name (`package:this`).
    pub root_package: String,
    /// What each node depends on.
    pub parents: Vec<Vec<usize>>,
    /// What depends on each node.
    pub children: Vec<Vec<usize>>,
}

impl Universe {
    /// Add a node; returns its index.
    pub fn push(&mut self, node: Node) -> usize {
        self.nodes.push(node);
        self.parents.push(Vec::new());
        self.children.push(Vec::new());
        self.nodes.len() - 1
    }

    /// `child` depends on `parent`.
    pub fn link(&mut self, child: usize, parent: usize) {
        if child == parent || self.parents[child].contains(&parent) {
            return;
        }
        self.parents[child].push(parent);
        self.children[parent].push(child);
    }

    /// The first node of a type and name.
    pub fn find(&self, resource_type: ResourceType, name: &str) -> Option<usize> {
        self.nodes
            .iter()
            .position(|n| n.resource_type == resource_type && n.name == name)
    }

    /// The source table node of (source, table).
    pub fn find_source(&self, source: &str, table: &str) -> Option<usize> {
        self.nodes.iter().position(|n| {
            n.source
                .as_ref()
                .is_some_and(|(s, t)| s == source && t == table)
        })
    }
}

pub use build::{build, Read};
