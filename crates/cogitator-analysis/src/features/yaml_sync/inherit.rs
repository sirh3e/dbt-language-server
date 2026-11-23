//! Where an undocumented column's description comes from: the column it
//! passes through upstream ([`ColumnLineage::direct`]), followed model by
//! model to the nearest documented ancestor, a model column or a source
//! column. A computed or renamed column inherits nothing; a hop whose
//! lineage is unclear stops the walk and says so.

use std::collections::{HashMap, HashSet};

use crate::context::ModelKind;
use crate::lineage::{ColumnLineage, ModelLineage, SourceRef};
use crate::DbtContext;

/// Hops followed before giving up (a project's longest chain is far shorter).
const MAX_HOPS: usize = 32;

/// A description found upstream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Found {
    pub description: String,
    /// The documented column it was read from.
    pub from: SourceRef,
    /// The undocumented models walked through on the way, nearest first.
    pub via: Vec<String>,
}

/// What the walk came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Walk {
    Found(Found),
    /// Not the same column upstream, or no documented ancestor.
    Nothing,
    /// A hop the sync cannot see through; the reason says which.
    Unclear(String),
}

/// Lineage per model, computed at most once per sync.
pub(super) struct Lineages<'a> {
    compute: &'a dyn Fn(&str) -> Option<ModelLineage>,
    memo: HashMap<String, Option<ModelLineage>>,
}

impl<'a> Lineages<'a> {
    pub fn new(compute: &'a dyn Fn(&str) -> Option<ModelLineage>) -> Self {
        Lineages {
            compute,
            memo: HashMap::new(),
        }
    }

    pub fn of(&mut self, model: &str) -> Option<&ModelLineage> {
        if !self.memo.contains_key(model) {
            let lineage = (self.compute)(model);
            self.memo.insert(model.to_string(), lineage); // boundary: the memo key
        }
        self.memo.get(model).and_then(Option::as_ref)
    }
}

/// The one upstream column `column` of `lineage` passes through unchanged.
pub(super) fn same_column<'l>(lineage: &'l ModelLineage, column: &str) -> Option<&'l SourceRef> {
    let col: &ColumnLineage = lineage
        .columns
        .iter()
        .find(|c| !c.name.contains('*') && c.name.eq_ignore_ascii_case(column))?;
    if !col.direct {
        return None;
    }
    let mut sources = col.sources.iter().filter(|s| !is_helper(s));
    let (Some(source), None) = (sources.next(), sources.next()) else {
        return None;
    };
    let upstream = match source {
        SourceRef::Model { column, .. }
        | SourceRef::Source { column, .. }
        | SourceRef::Unknown { column, .. } => column,
    };
    upstream.eq_ignore_ascii_case(column).then_some(source)
}

/// Columns the lineage invents for relation references in Jinja.
fn is_helper(source: &SourceRef) -> bool {
    matches!(source, SourceRef::Model { column, .. } if column.starts_with("__dbt_rel_"))
}

/// The description column `column` of `model` can inherit.
pub(super) fn walk(
    ctx: &DbtContext,
    lineages: &mut Lineages<'_>,
    model: &str,
    column: &str,
) -> Walk {
    let mut via: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut current = model.to_string(); // boundary: the walk owns its hop
    for _ in 0..MAX_HOPS {
        if !seen.insert(current.clone()) {
            return Walk::Unclear(format!("the lineage of '{model}' runs in a cycle"));
        }
        let Some(lineage) = lineages.of(&current) else {
            return Walk::Unclear(format!(
                "the column lineage of '{current}' is unclear (Jinja control flow \
                 without fresh compiled SQL, or SQL that does not parse)"
            ));
        };
        let Some(source) = same_column(lineage, column).cloned() else {
            return Walk::Nothing;
        };
        match &source {
            SourceRef::Model {
                name,
                column: upstream,
            } => {
                let Some(details) = ctx.model_detail_map.get(name) else {
                    return Walk::Unclear(format!(
                        "'{name}' is not a model, seed or snapshot the project knows \
                         (an unresolved ref, or a snapshot defined in YAML)"
                    ));
                };
                let documented = details
                    .columns
                    .iter()
                    .find(|c| c.name.eq_ignore_ascii_case(upstream))
                    .map(|c| c.description.trim())
                    .filter(|d| !d.is_empty());
                if let Some(description) = documented {
                    return Walk::Found(Found {
                        description: description.to_string(), // boundary: the plan owns it
                        from: source,
                        via,
                    });
                }
                if details.kind != ModelKind::Model {
                    return Walk::Nothing; // a seed or snapshot ends the lineage
                }
                via.push(name.clone()); // boundary: as above
                current = name.clone(); // boundary: the next hop
            }
            SourceRef::Source {
                source: s,
                table,
                column: upstream,
            } => {
                let documented = ctx
                    .source_detail_map
                    .get(s)
                    .and_then(|d| d.tables.get(table))
                    .and_then(|t| {
                        t.columns
                            .iter()
                            .find(|c| c.name.eq_ignore_ascii_case(upstream))
                    })
                    .map(|c| c.description.trim())
                    .filter(|d| !d.is_empty());
                return match documented {
                    Some(description) => Walk::Found(Found {
                        description: description.to_string(), // boundary: as above
                        from: source.clone(),
                        via,
                    }),
                    None => Walk::Nothing,
                };
            }
            SourceRef::Unknown { .. } => return Walk::Nothing,
        }
    }
    Walk::Unclear(format!("more than {MAX_HOPS} models upstream of '{model}'"))
}
