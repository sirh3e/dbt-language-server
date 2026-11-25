//! The lineage site's data, as serialized into the page and answered by
//! `dbt.lineageSite`. Field names are camelCase on the wire; ids are the
//! `dbt.projectGraph` ones (`<kind>:<name>`).

use serde::Serialize;

use crate::lineage::ModelLineage;

/// Bumped when the shape changes in a way the page must know about.
pub const FORMAT: u32 = 1;

/// The whole site: the project DAG with every node's columns and, for the
/// models cogitator could trace, where each output column comes from.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LineageSite {
    pub format: u32,
    pub project: String,
    /// `cogitator <version>`, said in the page's footer.
    pub generator: String,
    pub nodes: Vec<SiteNode>,
    pub edges: Vec<SiteEdge>,
    pub counts: Counts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SiteKind {
    Model,
    Seed,
    Snapshot,
    Source,
    Exposure,
    Metric,
    SemanticModel,
    Test,
    Analysis,
}

/// Whether, and how, a node's columns were traced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LineageState {
    /// Read off the model's template.
    Traced,
    /// Read off fresh `target/compiled` SQL (the template has control flow).
    Compiled,
    /// The model was not traced; `note` says why.
    Declined,
    /// Nothing to trace: a source, a seed, an exposure, …
    NotApplicable,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteNode {
    pub id: String,
    pub name: String,
    pub kind: SiteKind,
    /// Depth in the DAG (`dbt.projectGraph`'s layer: 0 has no parents).
    pub layer: usize,
    pub uri: Option<String>,
    /// The defining file relative to the project root, `/`-separated.
    pub path: Option<String>,
    pub package: Option<String>,
    pub materialized: Option<String>,
    pub description: String,
    pub tags: Vec<String>,
    /// The tests that depend on this node, by name, sorted.
    pub tests: Vec<String>,
    pub columns: Vec<SiteColumn>,
    pub lineage: LineageState,
    /// Why the lineage is what it is, for people (declined models).
    pub note: Option<String>,
}

/// How an output column is made from its inputs, as the schema job's
/// provenance says; `None` on the column when nothing says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Transform {
    /// One upstream column, unchanged, under the same name.
    Passthrough,
    /// One upstream column, unchanged, under another name.
    Renamed,
    /// A cast, function, aggregate, window, set operation or expression.
    Derived,
    /// A literal: no column feeds it.
    Constant,
    /// Opaque SQL, a subquery or a Jinja hole.
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteColumn {
    pub name: String,
    pub description: String,
    /// The YAML `data_type`, else the dialect spelling of a proven type.
    pub data_type: Option<String>,
    pub documented: bool,
    /// The model's SQL produces it (always false for sources and seeds).
    pub produced: bool,
    pub tests: Vec<String>,
    /// The columns this one reads, one hop up; empty when untraced.
    pub inputs: Vec<Hop>,
    /// An unexpanded `*` / `t.*` row: its columns are not known.
    pub star: bool,
    pub transform: Option<Transform>,
    /// The function or kind of expression behind a derived column.
    pub detail: Option<String>,
}

/// One upstream column.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hop {
    /// The node's id; `None` for a relation the project does not define.
    pub node: Option<String>,
    pub column: String,
    /// The relation as written, when `node` is `None`.
    pub relation: Option<String>,
}

/// `child` depends on `parent` (`dbt.projectGraph`'s `to` and `from`:
/// its edges run dependency → dependent).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SiteEdge {
    pub parent: String,
    pub child: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub nodes: usize,
    pub edges: usize,
    pub models: usize,
    pub sources: usize,
    pub seeds: usize,
    pub snapshots: usize,
    pub exposures: usize,
    pub models_traced: usize,
    pub models_declined: usize,
    /// Output columns of traced models, plus documented columns of
    /// declined ones.
    pub columns: usize,
    /// Of those, the ones with known inputs (or a literal).
    pub columns_traced: usize,
    pub columns_undetermined: usize,
}

/// The shell's answer for one model's lineage.
#[derive(Debug, Clone)]
pub enum Traced {
    Lineage(ModelLineage),
    /// Jinja control flow and no compiled SQL known fresh.
    ControlFlow,
    /// The SQL did not parse completely, or a cap cut it short.
    Incomplete,
    Unreadable,
}
