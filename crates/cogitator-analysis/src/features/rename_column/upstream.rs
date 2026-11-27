//! The upstream half of a column rename (`dbt.renameColumn` with
//! `direction: "both"`): from the model the cursor is in, up through every
//! model whose output passes the column on unchanged, to the model that
//! defines it. The rename then starts there and walks back down
//! ([`super::walk::walk_column`]), so the whole chain keeps one name.
//!
//! What counts as the same column is column lineage's
//! ([`crate::lineage::ColumnLineage::direct`]): a bare reference under the
//! name it already had, through CTEs, derived tables and `select *`, to
//! exactly one model. A `select *` over a model the lineage cannot see
//! into (no schema) is followed through the bound HIR. The climb stops at
//! a model that computes or re-aliases the column, at a source or seed
//! (whose columns are the warehouse's or the CSV's: the model reading it
//! gets `old as new`), and at an installed package's model. Anything the
//! HIR or lineage cannot answer — Jinja control blocks, a column from two
//! relations, a model that does not spell the column — is a refusal.

use cogitator_dialects::Dialect;
use cogitator_ir::hir::{FromKind, RelTarget};
use cogitator_parser::ParsedDocument;
use serde::Serialize;

use super::super::references::files::Reader;
use super::super::DbtContext;
use super::seed::Defined;
use super::sql::{bound, lower, Walk};
use super::walk::read_parsed;
use crate::context::ModelKind;
use crate::lineage::{column_lineage, SourceRef};

/// One step up from a model's output column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Hop {
    /// Passed through from this model's column of the same name.
    Model { name: String, column: String },
    /// Passed through from a source table.
    Source { source: String, table: String },
    /// The model defines it: an alias, an expression, a raw table.
    Defined,
}

/// Where the climb ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Origin {
    /// The model the rename starts from.
    pub model: String,
    /// The column's name there.
    pub column: String,
    /// The models climbed, from the cursor's up to `model`.
    pub chain: Vec<String>,
    /// Why the climb stopped at `model`.
    pub stop: String,
}

/// Longer chains are a refusal, not a slow request.
const MAX_HOPS: usize = 64;

/// The model a rename of `model.column` up- and downstream starts from.
pub(crate) fn origin_of(
    ctx: &DbtContext,
    reader: &Reader<'_>,
    model: &str,
    column: &str,
) -> Result<Origin, String> {
    let mut chain = vec![model.to_string()];
    let mut column = column.to_string();
    loop {
        let current = chain.last().cloned().unwrap_or_default();
        let details = ctx
            .model_detail_map
            .get(&current)
            .ok_or_else(|| format!("`{current}` is not a model of this project"))?;
        let path = details.uri.display().to_string();
        let parsed = read_parsed(ctx, reader, &details.uri)
            .ok_or_else(|| format!("{path}: cannot be read"))?;
        let hop = hop(&parsed, &ctx.dialect, &column).map_err(|why| format!("{path}: {why}"))?;
        let stop = match hop {
            Hop::Defined => format!("`{current}` defines `{column}`"),
            Hop::Source { source, table } => format!(
                "`{current}` reads it from the source `{source}.{table}`, whose column stays; `{current}` gets an alias"
            ),
            Hop::Model { name, column: up } => match stop_before(ctx, &name) {
                Some(why) => format!("{why}; `{current}` gets an alias"),
                None if chain.contains(&name) => {
                    return Err(format!("`{name}` reads itself through `{current}`"))
                }
                None if chain.len() >= MAX_HOPS => {
                    return Err("the column passes through too many models".into())
                }
                None => {
                    chain.push(name);
                    column = up;
                    continue;
                }
            },
        };
        return Ok(Origin {
            model: current,
            column,
            chain,
            stop,
        });
    }
}

/// Why the climb must not enter `model`, if it must not.
fn stop_before(ctx: &DbtContext, model: &str) -> Option<String> {
    let Some(details) = ctx.model_detail_map.get(model) else {
        return Some(format!("`{model}` is not a model of this project"));
    };
    if details.project_name != ctx.project.name {
        return Some(format!(
            "`{model}` belongs to the installed package `{}`",
            details.project_name
        ));
    }
    match details.kind {
        ModelKind::Model => None,
        ModelKind::Seed => Some(format!("`{model}` is a seed, whose columns are its CSV's")),
        ModelKind::Snapshot => Some(format!("`{model}` is a snapshot")),
    }
}

/// Where one model's output column `column` comes from.
pub(crate) fn hop(parsed: &ParsedDocument, dialect: &Dialect, column: &str) -> Result<Hop, String> {
    let lowered = lower(parsed, dialect)?;
    let lineage = column_lineage(parsed, dialect).ok_or("its SQL does not parse completely")?;
    let named: Vec<_> = lineage
        .columns
        .iter()
        .filter(|c| c.name.eq_ignore_ascii_case(column))
        .collect();
    match named.as_slice() {
        [] => {}
        [one] if !one.direct => return Ok(Hop::Defined),
        [one] => {
            return match one.sources.as_slice() {
                [SourceRef::Model { name, column: up }] if up.eq_ignore_ascii_case(column) => {
                    Ok(Hop::Model {
                        name: name.clone(),
                        column: up.clone(),
                    })
                }
                [SourceRef::Source { source, table, .. }] => Ok(Hop::Source {
                    source: source.clone(),
                    table: table.clone(),
                }),
                [SourceRef::Unknown { .. }] | [] => Ok(Hop::Defined),
                many => Err(format!(
                    "`{column}` comes from {} relations; rename it by hand",
                    many.len()
                )),
            }
        }
        _ => return Err(format!("it outputs `{column}` more than once")),
    }
    // not named: only a `*` over a relation outside the model can pass it
    let (hir, root) = bound(&lowered)?;
    let walk = Walk::new(hir, &parsed.text, column);
    let froms = match walk.defined(root)? {
        Defined::Item(None) => return Ok(Hop::Defined),
        // a bare column of an in-model query that only `*` fills
        Defined::Item(Some(from)) => match walk.inner_query(from) {
            Some(inner) => walk.upstream_stars(inner, 0),
            None => vec![from],
        },
        Defined::Stars(froms) => froms,
    };
    let mut hops: Vec<Hop> = froms
        .iter()
        .filter_map(|f| match &hir.from.get(*f)?.kind {
            FromKind::Base { target, .. } => Some(target),
            _ => None,
        })
        .map(|target| match target {
            RelTarget::Model { name, .. } => Hop::Model {
                name: hir.syms.text(*name).to_string(),
                column: column.to_string(),
            },
            RelTarget::Source { source, table } => Hop::Source {
                source: hir.syms.text(*source).to_string(),
                table: hir.syms.text(*table).to_string(),
            },
            _ => Hop::Defined,
        })
        .collect();
    hops.dedup();
    match hops.as_slice() {
        [one] => Ok(one.clone()),
        [] => Ok(Hop::Defined),
        _ => Err(format!(
            "`{column}` comes in through `select *` over {} relations; rename it by hand",
            froms.len()
        )),
    }
}
