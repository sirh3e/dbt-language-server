//! dbt.diffAt: a model now against itself at another revision — a git
//! revision, or the last dbt run (`rev: "last-run"`, from
//! target/manifest.json and target/compiled/). Three views:
//! - `sql`: the template text;
//! - `compiled`: both sides rendered with dbt.expandJinja's in-process
//!   renderer (the same stubs, so the diff shows what the template
//!   change does, not what the warehouse would see); for `last-run`
//!   the old side is what dbt actually compiled;
//! - `lineage`: the column lineage of both sides, as added / removed /
//!   changed-source columns and a diff of one line per column.
//!
//! Pure: the shell fetches the old text (git, the manifest) and the
//! current one and assembles the [`PreviewInputs`].

mod lineage;

use cogitator_dialects::Dialect;
use cogitator_error::guidance::Guidance;
use serde::Serialize;
use similar::TextDiff;

pub use lineage::{lineage_diff, lineage_text, ChangedColumn, LineageDiff};

use super::jinja_preview::{expand_jinja, PreviewInputs};

pub const LAST_RUN: &str = "last-run";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DiffWhat {
    Sql,
    Compiled,
    Lineage,
}

impl DiffWhat {
    pub fn parse(text: Option<&str>) -> Result<DiffWhat, Guidance> {
        match text {
            None | Some("sql") => Ok(DiffWhat::Sql),
            Some("compiled") => Ok(DiffWhat::Compiled),
            Some("lineage") => Ok(DiffWhat::Lineage),
            Some(other) => Err(
                Guidance::new(format!("'{other}' is not a dbt.diffAt view."))
                    .suggest(cogitator_error::guidance::closest_matches(
                        other,
                        ["sql", "compiled", "lineage"],
                    ))
                    .fix("pass what: \"sql\", \"compiled\" or \"lineage\"."),
            ),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            DiffWhat::Sql => "sql",
            DiffWhat::Compiled => "compiled",
            DiffWhat::Lineage => "lineage",
        }
    }
}

/// The old side as the shell found it.
#[derive(Debug, Clone)]
pub enum OldSide {
    /// The model's template at the revision.
    Template(String),
    /// SQL dbt already compiled (`last-run`, `what: compiled`).
    Compiled(String),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffAt {
    pub model: String,
    pub rev: String,
    pub what: DiffWhat,
    pub before: String,
    pub after: String,
    /// Unified diff, rev → now; empty when identical.
    pub diff: String,
    pub identical: bool,
    /// `what: lineage` only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub columns: Option<LineageDiff>,
    /// What the comparison rests on, when it is not plain text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

pub struct DiffRequest<'a> {
    pub model: &'a str,
    pub rev: &'a str,
    pub what: DiffWhat,
    pub old: OldSide,
    pub current: &'a str,
    pub dialect: &'a Dialect,
}

pub fn diff_at(req: DiffRequest<'_>, inputs: &PreviewInputs) -> Result<DiffAt, Guidance> {
    let mut note = None;
    let mut columns = None;
    let (before, after) = match (req.what, &req.old) {
        (DiffWhat::Sql, OldSide::Template(old)) => (old.clone(), req.current.to_string()),
        (DiffWhat::Compiled, OldSide::Template(old)) => {
            note = Some("both sides rendered in-process with dbt.expandJinja's stubs".to_string());
            (render(old, inputs)?, render(req.current, inputs)?)
        }
        (DiffWhat::Compiled, OldSide::Compiled(old)) => {
            note = Some(
                "the old side is what dbt compiled; the current one is rendered in-process"
                    .to_string(),
            );
            (settled(old), render(req.current, inputs)?)
        }
        (DiffWhat::Lineage, OldSide::Template(old)) => {
            let old_lineage = lineage_of(old, req.dialect);
            let new_lineage = lineage_of(req.current, req.dialect);
            columns = Some(lineage_diff(old_lineage.as_ref(), new_lineage.as_ref()));
            (
                lineage_text(old_lineage.as_ref()),
                lineage_text(new_lineage.as_ref()),
            )
        }
        (_, OldSide::Compiled(_)) => {
            return Err(
                Guidance::new("only the compiled view can compare against compiled SQL.")
                    .fix("pass the model's template as the old side."),
            )
        }
    };
    let identical = before == after;
    let diff = if identical {
        String::new()
    } else {
        TextDiff::from_lines(&before, &after)
            .unified_diff()
            .context_radius(3)
            .header(
                &format!("{} @ {} ({})", req.model, req.rev, req.what.as_str()),
                &format!("{} (now, {})", req.model, req.what.as_str()),
            )
            .to_string()
    };
    Ok(DiffAt {
        model: req.model.to_string(),
        rev: req.rev.to_string(),
        what: req.what,
        before,
        after,
        diff,
        identical,
        columns,
        note,
    })
}

fn render(body: &str, inputs: &PreviewInputs) -> Result<String, Guidance> {
    Ok(settled(&expand_jinja(body, inputs)?.sql))
}

/// Trailing whitespace is noise between a renderer and dbt's file.
fn settled(sql: &str) -> String {
    format!("{}\n", sql.trim_end())
}

fn lineage_of(text: &str, dialect: &Dialect) -> Option<crate::lineage::ModelLineage> {
    let parsed = cogitator_parser::pipeline::parse_document(text, dialect).ok()?;
    crate::lineage::column_lineage(&parsed, dialect)
}

/// The text the `show: true` document holds.
pub fn shown_text(diff: &DiffAt) -> String {
    let mut out = String::new();
    if let Some(note) = &diff.note {
        out.push_str(&format!("-- {note}\n"));
    }
    if let Some(columns) = &diff.columns {
        out.push_str(&columns_summary(columns));
    }
    if diff.identical {
        out.push_str(&format!(
            "-- {}: no {} difference between {} and now.\n",
            diff.model,
            diff.what.as_str(),
            diff.rev
        ));
    } else {
        out.push_str(&diff.diff);
    }
    out
}

fn columns_summary(columns: &LineageDiff) -> String {
    if let Some(why) = &columns.unknown {
        return format!("-- lineage unknown: {why}\n");
    }
    let mut out = String::new();
    for name in &columns.added {
        out.push_str(&format!("-- + column {name}\n"));
    }
    for name in &columns.removed {
        out.push_str(&format!("-- - column {name}\n"));
    }
    for changed in &columns.changed {
        out.push_str(&format!(
            "-- ~ column {}: {} → {}\n",
            changed.name,
            changed.before.join(", "),
            changed.after.join(", ")
        ));
    }
    out
}

#[cfg(test)]
mod tests;
