//! textDocument/rename and prepareRename for a model's output column,
//! project-wide and transitive: the model's own select item, its YAML
//! entry and the lists that name it, and every consumer at any depth that
//! keeps the name ([`walk::walk_column`], which column references share).
//! One WorkspaceEdit, or a refusal naming the files the walk cannot
//! follow — never a partial edit.
//!
//! The cursor names the column on its `columns:` YAML entry, or on a SQL
//! word of the model's file that is one of its output columns (or a
//! documented column of a model the file selects from).
//!
//! That is [`Direction::Down`], what textDocument/rename does: a column the
//! model passes through from upstream gets `old as new` there and its
//! upstream model is left alone. [`Direction::Both`] (`dbt.renameColumn`)
//! first climbs the column's lineage to the model that defines it
//! ([`upstream::origin_of`]) and renames from there, so every model it
//! passes through keeps one name.

mod refs;
mod seed;
mod semantic;
mod snapshots;
mod sql;
mod upstream;
pub(crate) mod walk;
mod yaml;
mod yaml_lines;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cogitator_error::guidance::Guidance;
use cogitator_lsp::{
    DocumentChange, Position, PrepareRenameResult, Range, TextDocumentEdit, TextEdit, WorkspaceEdit,
};
use cogitator_parser::TokenKind;
use serde::Serialize;

use super::references::files::{is_yaml, path_of, Reader};
use super::references::occur::{contains, name_span, unquoted};
use super::references::target::{target_at, Target};
use super::{file_uri, model_name_from_uri, token_range, DbtContext, Document};
use walk::{walk_column, ColumnWalk, Edit};

pub use upstream::Origin;

/// The column the cursor names: (model, column, the range under the cursor).
pub(crate) fn column_at(
    ctx: &DbtContext,
    doc: &Document,
    uri: &str,
    pos: Position,
) -> Option<(String, String, Range)> {
    let under = cursor_range(ctx, doc, uri, pos)?;
    if let Some(Target::Column { model, column }) = target_at(ctx, doc, uri, pos) {
        return Some((model, column, under));
    }
    // an undocumented output column of the file's own model
    let model = model_name_from_uri(uri).filter(|m| ctx.model_detail_map.contains_key(m))?;
    let idx = doc.parsed.find_token_at_cursor(pos.line, pos.character)?;
    let word = unquoted(doc.parsed.literal(&doc.parsed.tokens[idx]));
    let lineage = crate::lineage::column_lineage(&doc.parsed, &ctx.dialect)?;
    let column = lineage
        .columns
        .iter()
        .find(|c| c.name.eq_ignore_ascii_case(word))?
        .name
        .clone();
    Some((model, column, under))
}

/// The SQL word or YAML column name under the cursor.
fn cursor_range(ctx: &DbtContext, doc: &Document, uri: &str, pos: Position) -> Option<Range> {
    let path = path_of(uri)?;
    if is_yaml(&path) {
        return ctx
            .model_detail_map
            .values()
            .filter(|d| d.schema_uri.as_deref() == Some(path.as_path()))
            .flat_map(|d| &d.columns)
            .filter_map(|c| name_span(&doc.text, c.name_pos, &c.name))
            .find(|r| contains(r, pos));
    }
    let idx = doc.parsed.find_token_at_cursor(pos.line, pos.character)?;
    let token = &doc.parsed.tokens[idx];
    matches!(token.kind, TokenKind::Word(_) | TokenKind::Ident).then(|| token_range(token))
}

/// prepareRename: the column's range and name, `None` off a column.
pub fn prepare_rename_column(
    ctx: &DbtContext,
    doc: Option<&Document>,
    uri: &str,
    pos: Position,
) -> Option<PrepareRenameResult> {
    let (_, column, range) = column_at(ctx, doc?, uri, pos)?;
    Some(PrepareRenameResult {
        range,
        placeholder: column,
    })
}

/// rename: `None` when the cursor is on no column, else the edit or the
/// user-facing refusal.
pub fn rename_column_at(
    ctx: &DbtContext,
    doc: Option<&Document>,
    uri: &str,
    pos: Position,
    new_name: &str,
    read: &dyn Fn(&Path) -> Option<String>,
) -> Option<Result<WorkspaceEdit, String>> {
    let (model, column, _) = column_at(ctx, doc?, uri, pos)?;
    let reader = Reader::with(doc.map(|d| (uri, &*d.text)), read);
    Some(rename_column(ctx, &reader, &model, &column, new_name))
}

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
        Direction::Both => upstream::origin_of(ctx, reader, model, column).map_err(|why| {
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

pub(crate) fn valid_column_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub(crate) fn rename_column(
    ctx: &DbtContext,
    reader: &Reader<'_>,
    model: &str,
    column: &str,
    new_name: &str,
) -> Result<WorkspaceEdit, String> {
    rename_walk(ctx, reader, model, column, new_name).map(|(edit, _)| edit)
}

/// The edit, and the models whose output column it renames.
fn rename_walk(
    ctx: &DbtContext,
    reader: &Reader<'_>,
    model: &str,
    column: &str,
    new_name: &str,
) -> Result<(WorkspaceEdit, Vec<String>), String> {
    if !valid_column_name(new_name) {
        return Err(
            Guidance::new(format!("`{new_name}` is not a plain column name."))
                .fix("use letters, digits and underscores, not starting with a digit.")
                .to_message(),
        );
    }
    if new_name == column {
        let edit = WorkspaceEdit {
            document_changes: Vec::new(),
        };
        return Ok((edit, Vec::new()));
    }
    let walk = walk_column(ctx, reader, model, column);
    refuse(&walk, model, column, new_name)?;
    Ok((workspace_edit(&walk, new_name), walk.models.clone()))
}

fn refuse(walk: &ColumnWalk, model: &str, column: &str, new_name: &str) -> Result<(), String> {
    if !walk.refusals.is_empty() {
        return Err(Guidance::new(format!(
            "`{model}.{column}` cannot be renamed safely: {}.",
            walk.refusals.join("; ")
        ))
        .fix("rewrite those files by hand, or simplify them (no Jinja control blocks, qualified columns) and rename again.")
        .to_message());
    }
    let wanted = new_name.to_ascii_lowercase();
    let case_only = wanted == column.to_ascii_lowercase();
    if let Some((owner, _)) = walk.taken.iter().find(|(_, n)| !case_only && *n == wanted) {
        return Err(Guidance::new(format!(
            "`{new_name}` is already a column of `{owner}`, which the rename reaches."
        ))
        .fix("pick another name, or rename that column first.")
        .to_message());
    }
    Ok(())
}

fn workspace_edit(walk: &ColumnWalk, new_name: &str) -> WorkspaceEdit {
    let mut by_file: BTreeMap<PathBuf, Vec<TextEdit>> = BTreeMap::new();
    for site in &walk.sites {
        let edit = match site.edit {
            Edit::Name { quote } => TextEdit {
                range: site.range,
                new_text: quoted(new_name, quote),
            },
            Edit::AliasAfter => TextEdit {
                range: Range::zero_width(site.range.end),
                new_text: format!(" as {new_name}"),
            },
            Edit::ExprLine { at, indent } => TextEdit {
                range: Range::zero_width(at),
                new_text: format!("\n{}expr: {new_name}", " ".repeat(indent)),
            },
        };
        by_file.entry(site.path.clone()).or_default().push(edit);
    }
    let document_changes = by_file
        .into_iter()
        .map(|(path, mut edits)| {
            edits.sort_by_key(|e| (e.range.start.line, e.range.start.character));
            edits.dedup_by(|a, b| a.range == b.range);
            DocumentChange::Edit(TextDocumentEdit::new(file_uri(&path), edits))
        })
        .collect();
    WorkspaceEdit { document_changes }
}

fn quoted(name: &str, quote: Option<char>) -> String {
    match quote {
        Some('[') => format!("[{name}]"),
        Some(q) => format!("{q}{name}{q}"),
        None => name.to_string(),
    }
}
