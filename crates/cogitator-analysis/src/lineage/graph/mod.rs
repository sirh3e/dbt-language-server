//! The project's column lineage as one petgraph graph: every model's output
//! columns and the source and unknown columns they read, an edge from each
//! upstream column to the column it feeds.
//!
//! Built from the per-model [`ModelLineage`]s, which say what one model's
//! columns read; the graph is what answers across models (upstream,
//! downstream, the closure of several columns, the same-name origin of a
//! column), so the features that walk lineage share one structure instead
//! of each building its own maps.
//!
//! What every walker had to repeat is decided here, once, while building:
//! - column names match case-insensitively (`Customer_ID` is `customer_id`),
//!   relation names exactly; a model's column is spelled as the model
//!   writes it, any other column as it was first seen;
//! - dbt's internal `__dbt_rel_*` placeholders are no columns and get no
//!   node;
//! - a `*` row is resolved, along [`Flow::Star`] edges, into the columns it
//!   stands for, each only where the model does not list that column itself:
//!   - a named source (`* <- stg.id, stg.name`, what an untyped lineage
//!     writes for an unexpanded `select *`) is a column the model passes on
//!     under its own name, as `dbt.impact` and lineage's `substituted` read it;
//!   - an input's own `*` (`* <- stg.*`, the rest of an open input) gives the
//!     model every column of that input model, until no star adds a column:
//!     chains of `select *` resolve, a cycle of stars stops when nothing is new.
//!
//! Pure: no I/O, no cache. Queries are in `query.rs`.

mod query;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashMap};

pub use petgraph::stable_graph::NodeIndex;
use petgraph::stable_graph::StableDiGraph;

use super::{ModelLineage, SourceRef};

pub use query::Reached;

/// What a column belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Relation {
    /// A dbt model (`ref('…')`).
    Model(String),
    /// A dbt source table (`source('a', 'b')`).
    Source { source: String, table: String },
    /// A relation the project does not define (a raw table, an outer CTE).
    Unknown(String),
}

/// One column of one relation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnNode {
    pub relation: Relation,
    /// As first seen (lineage, or a source reference).
    pub column: String,
}

/// How an upstream column reaches the column it feeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Flow {
    /// Passed on unchanged under the same name ([`super::ColumnLineage::direct`]).
    Direct,
    /// Computed from it, or renamed.
    Derived,
    /// Passed on by a `*` the model selects from its input.
    Star,
}

/// The name a `*` row and a star source use.
const STAR: &str = "*";

type Key = (Relation, String);

/// See the module documentation.
#[derive(Debug, Default)]
pub struct ColumnGraph {
    graph: StableDiGraph<ColumnNode, Flow>,
    index: HashMap<Key, NodeIndex>,
    /// Each model's output columns, in its lineage's order, the columns a
    /// star resolved after them.
    models: BTreeMap<String, Vec<NodeIndex>>,
}

impl ColumnGraph {
    /// The graph of these models' lineages. A model listed twice keeps its
    /// first lineage.
    pub fn build<'a>(models: impl IntoIterator<Item = (&'a str, &'a ModelLineage)>) -> Self {
        let mut graph = ColumnGraph::default();
        let mut stars: BTreeMap<String, Vec<String>> = BTreeMap::new();
        // a `*` row's named sources: columns the model passes on under their
        // own name, unless it lists them itself (known once it is all read)
        let mut passed: Vec<(String, NodeIndex)> = Vec::new();
        for (model, lineage) in models {
            if graph.models.contains_key(model) {
                continue;
            }
            graph.models.insert(model.to_string(), Vec::new());
            for column in &lineage.columns {
                if column.name == STAR {
                    graph.star_row(model, &column.sources, &mut stars, &mut passed);
                    continue;
                }
                let target = graph.output(model, &column.name);
                let flow = if column.direct {
                    Flow::Direct
                } else {
                    Flow::Derived
                };
                for (relation, name) in column.sources.iter().filter_map(upstream_of) {
                    let from = graph.node(relation, name);
                    graph.connect(from, target, flow);
                }
            }
        }
        for (model, from) in passed {
            let name = graph.graph[from].column.clone();
            if !graph.outputs_column(&model, &name) {
                let to = graph.output(&model, &name);
                graph.connect(from, to, Flow::Star);
            }
        }
        graph.resolve_stars(&stars);
        graph
    }

    /// A `*` row: an input's own `*` (the rest of its columns) is the
    /// model's `*` column and resolved later; a named source is passed on.
    fn star_row(
        &mut self,
        model: &str,
        sources: &[SourceRef],
        stars: &mut BTreeMap<String, Vec<String>>,
        passed: &mut Vec<(String, NodeIndex)>,
    ) {
        for (relation, name) in sources.iter().filter_map(upstream_of) {
            if name == STAR {
                if let Relation::Model(input) = &relation {
                    stars
                        .entry(model.to_string())
                        .or_default()
                        .push(input.clone());
                }
                let from = self.node(relation, name);
                let target = self.output(model, STAR);
                self.connect(from, target, Flow::Star);
            } else {
                let from = self.node(relation, name);
                passed.push((model.to_string(), from));
            }
        }
    }

    /// The model's output column `name`, made if new. A model spells its
    /// own columns: a node a downstream reference made first takes the
    /// model's spelling when the model outputs it.
    fn output(&mut self, model: &str, name: &str) -> NodeIndex {
        let node = self.node(Relation::Model(model.to_string()), name);
        let outputs = self.models.entry(model.to_string()).or_default();
        if !outputs.contains(&node) {
            outputs.push(node);
            self.graph[node].column = name.to_string();
        }
        node
    }

    fn outputs_column(&self, model: &str, name: &str) -> bool {
        self.models.get(model).is_some_and(|outs| {
            outs.iter()
                .any(|&n| self.graph[n].column.eq_ignore_ascii_case(name))
        })
    }

    fn node(&mut self, relation: Relation, name: &str) -> NodeIndex {
        let key = (relation, name.to_lowercase());
        if let Some(&node) = self.index.get(&key) {
            return node;
        }
        let node = self.graph.add_node(ColumnNode {
            relation: key.0.clone(),
            column: name.to_string(),
        });
        self.index.insert(key, node);
        node
    }

    /// One edge per pair: a column read twice is one dependency, its
    /// strongest flow wins (Direct over Star over Derived).
    fn connect(&mut self, from: NodeIndex, to: NodeIndex, flow: Flow) {
        if from == to {
            return;
        }
        match self.graph.find_edge(from, to) {
            Some(edge) => {
                let kept = self.graph[edge];
                if rank(flow) > rank(kept) {
                    self.graph[edge] = flow;
                }
            }
            None => {
                self.graph.add_edge(from, to, flow);
            }
        }
    }

    /// Every model with a `*` row gains the columns of each input it passes
    /// on that it does not list, until a round adds none.
    fn resolve_stars(&mut self, stars: &BTreeMap<String, Vec<String>>) {
        loop {
            let mut added = false;
            for (model, inputs) in stars {
                for input in inputs {
                    let columns: Vec<NodeIndex> = self
                        .models
                        .get(input)
                        .into_iter()
                        .flatten()
                        .copied()
                        .filter(|&n| self.graph[n].column != STAR)
                        .collect();
                    for from in columns {
                        let name = self.graph[from].column.clone();
                        // a downstream reference may have made the node
                        // already: what counts is whether the model outputs it
                        if self.outputs_column(model, &name) {
                            continue; // listed by the model, or resolved already
                        }
                        let to = self.output(model, &name);
                        self.connect(from, to, Flow::Star);
                        added = true;
                    }
                }
            }
            if !added {
                return;
            }
        }
    }

    /// How many columns the graph holds.
    pub fn len(&self) -> usize {
        self.graph.node_count()
    }

    pub fn is_empty(&self) -> bool {
        self.graph.node_count() == 0
    }
}

fn rank(flow: Flow) -> u8 {
    match flow {
        Flow::Derived => 0,
        Flow::Star => 1,
        Flow::Direct => 2,
    }
}

/// The relation and column a source names; `None` for dbt's internal
/// relation placeholders.
fn upstream_of(source: &SourceRef) -> Option<(Relation, &str)> {
    if source.is_placeholder() {
        return None;
    }
    let relation = match source {
        SourceRef::Model { name, .. } => Relation::Model(name.clone()),
        SourceRef::Source { source, table, .. } => Relation::Source {
            source: source.clone(),
            table: table.clone(),
        },
        SourceRef::Unknown { name, .. } => Relation::Unknown(name.clone()),
    };
    Some((relation, source.column()))
}
