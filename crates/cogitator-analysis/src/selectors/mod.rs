//! dbt's node selection language, one engine for every feature that reads
//! a selector: `workspace/symbol`'s `tag:`/`selector:` queries, the run
//! lenses, selectors.yml diagnostics, completion, hover and lenses, and
//! `dbt.select`. Pure: the shell hands in a `read` closure for the files
//! a [`Universe`] is built from. What dbt supports that this does not is
//! listed in docs/selectors.md.

mod definition;
pub mod eval;
pub mod glob;
mod indirect;
mod matcher;
pub mod method;
pub mod syntax;
pub mod universe;
pub mod yaml;

pub use eval::{Evaluation, Indirect, Issue};
pub use method::{Method, METHODS};
pub use syntax::{parse, union_of, Criterion, Selection, SyntaxError};
pub use universe::{Node, Read, ResourceType, Universe};
pub use yaml::{Definition, Expr, SelectorsFile};

use crate::DbtContext;

/// Every selectable node of the project.
pub fn universe(ctx: &DbtContext, read: &Read<'_>) -> Universe {
    universe::build(ctx, read)
}

/// The nodes a `--select` string picks, as `dbt ls --select` lists them
/// (tests come along by eager indirect selection); `selector:<name>`
/// criteria resolve against `defs`.
pub fn select(
    u: &Universe,
    defs: &[Definition],
    text: &str,
    indirect: Indirect,
) -> Result<Evaluation, SyntaxError> {
    let selection = syntax::parse(text)?;
    Ok(eval::select(u, defs, &selection, indirect))
}

/// The nodes a selectors.yml definition picks.
pub fn select_definition(u: &Universe, defs: &[Definition], name: &str) -> Evaluation {
    eval::select_definition(u, defs, name)
}

/// Nodes in a stable order for display: by resource type, then name.
pub fn sorted<'a>(u: &'a Universe, eval: &Evaluation) -> Vec<&'a Node> {
    let mut nodes: Vec<&Node> = eval.nodes.iter().map(|&i| &u.nodes[i]).collect();
    nodes.sort_by(|a, b| {
        (a.resource_type, a.display_name()).cmp(&(b.resource_type, b.display_name()))
    });
    nodes
}

/// An issue in words.
pub fn issue_message(issue: &Issue) -> String {
    match issue {
        Issue::UnknownMethod { name, .. } => format!(
            "unknown selector method `{name}` (known: {})",
            METHODS
                .iter()
                .map(|(m, _)| *m)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Issue::UnknownSelector { name, .. } => {
            format!("selectors.yml defines no selector `{name}`")
        }
        Issue::Cycle { chain, .. } => {
            format!("selector cycle: {}", chain.join(" → "))
        }
        Issue::NeedsState { method, .. } => format!(
            "`{method}:` needs a previous run's artifacts (dbt --state); it selects nothing here"
        ),
    }
}

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;
