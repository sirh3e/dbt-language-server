//! `dbt.renameColumn`: a column rename in the direction asked for
//! ([`Direction`]), with where it started ([`Origin`]) and the models it
//! renames, for the client to preview.

use std::path::Path;

use cogitator_error::guidance::Guidance;
use cogitator_lsp::{Position, WorkspaceEdit};
use serde::Serialize;

use super::super::references::files::Reader;
use super::super::{DbtContext, Document};
use super::upstream::{origin_of, Origin};
use super::{column_at, rename_walk};

/// Which way a column rename reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// The model and every consumer that keeps the name.
    Down,
    /// The model the column comes from, then every consumer from there.
    Both,
}

impl Direction {
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "down" | "downstream" => Some(Direction::Down),
            "both" | "up" | "upstream" => Some(Direction::Both),
            _ => None,
        }
    }
}

/// What `dbt.renameColumn` answers.
#[derive(Debug, Clone, Serialize)]
pub struct ColumnRename {
    pub model: String,
    pub column: String,
    #[serde(rename = "newName")]
    pub new_name: String,
    pub direction: Direction,
    /// Where an up- and downstream rename started, and why there.
    pub origin: Origin,
    /// Every model whose output column is renamed, the origin first.
    pub models: Vec<String>,
    pub edit: WorkspaceEdit,
}

/// `dbt.renameColumn`: `None` when the cursor is on no column, else the
/// rename or the user-facing refusal.
pub fn rename_column_directed(
    ctx: &DbtContext,
    doc: Option<&Document>,
    uri: &str,
    pos: Position,
    new_name: &str,
    direction: Direction,
    read: &dyn Fn(&Path) -> Option<String>,
) -> Option<Result<ColumnRename, String>> {
    let (model, column, _) = column_at(ctx, doc?, uri, pos)?;
    let reader = Reader::with(doc.map(|d| (uri, &*d.text)), read);
    Some(directed(ctx, &reader, &model, &column, new_name, direction))
}

fn directed(
    ctx: &DbtContext,
    reader: &Reader<'_>,
    model: &str,
    column: &str,
    new_name: &str,
    direction: Direction,
) -> Result<ColumnRename, String> {
    let origin = match direction {
        Direction::Down => Origin {
            model: model.to_string(),
            column: column.to_string(),
            chain: vec![model.to_string()],
            stop: "a downstream rename starts here".into(),
        },
        Direction::Both => origin_of(ctx, reader, model, column).map_err(|why| {
            Guidance::new(format!(
                "`{model}.{column}` cannot be traced upstream: {why}."
            ))
            .fix("rename it downstream only (textDocument/rename), or simplify that model.")
            .to_message()
        })?,
    };
    let (edit, models) = rename_walk(ctx, reader, &origin.model, &origin.column, new_name)?;
    if !models.is_empty() && !models.iter().any(|m| m == model) {
        return Err(Guidance::new(format!(
            "renaming `{}.{}` would not reach `{model}`.",
            origin.model, origin.column
        ))
        .fix("rename the column in each model by hand.")
        .to_message());
    }
    Ok(ColumnRename {
        model: model.to_string(),
        column: column.to_string(),
        new_name: new_name.to_string(),
        direction,
        origin,
        models,
        edit,
    })
}
