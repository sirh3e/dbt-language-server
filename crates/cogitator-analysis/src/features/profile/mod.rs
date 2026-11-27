//! Column profiling: a sampled profile of one output column of a model
//! (row count, distinct count, null rate, min/max, the ten most common
//! values) from the model's relation in the warehouse. Off by default
//! (cogitator.yml `warehouse.profile`): it queries the warehouse.
//! `dbt.profileColumn` runs the query in a background job and fills a
//! cache; the hover only reads that cache. Pure here: the query text,
//! reading the result rows, the cache key and the hover text.

mod query;
mod render;

use cogitator_error::guidance::Guidance;
use cogitator_lsp::Position;
use serde::Serialize;
use serde_json::Value;

pub use query::{profile_sql, quote_identifier, TOP_VALUES};
pub use render::{hover_markdown, sparkline};

use crate::lineage::ModelLineage;
use crate::DbtContext;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopValue {
    pub value: String,
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnProfile {
    pub model: String,
    pub column: String,
    pub relation: String,
    /// Rows in the sample.
    pub rows: u64,
    pub distinct: u64,
    pub nulls: u64,
    pub null_rate: f64,
    pub min: Option<String>,
    pub max: Option<String>,
    pub top: Vec<TopValue>,
    /// `duckdb` or `dbt show`.
    pub engine: String,
    /// What the profile is current for: the last run's `generated_at`,
    /// or empty when target/run_results.json had none.
    pub stamp: String,
    pub sql: String,
}

impl ColumnProfile {
    pub fn stamp_note(&self) -> String {
        if self.stamp.is_empty() {
            "no dbt run recorded".to_string()
        } else {
            format!("as of the run of {}", self.stamp)
        }
    }
}

/// What the cache is keyed by: one column of one relation. The entry
/// carries its stamp; a profile of another run's table is not shown.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProfileKey {
    pub model: String,
    /// Lowercased: warehouses fold unquoted names.
    pub column: String,
}

impl ProfileKey {
    pub fn new(model: &str, column: &str) -> Self {
        ProfileKey {
            model: model.to_string(),
            column: column.to_ascii_lowercase(),
        }
    }
}

/// The output column of the model under the cursor: the identifier at
/// `pos` when it names a column the model produces (lineage) or
/// documents (YAML). `None` otherwise — a profile is of the model's own
/// relation, never guessed for another relation's column.
pub fn profile_target(
    ctx: &DbtContext,
    model: &str,
    doc: &crate::Document,
    lineage: Option<&ModelLineage>,
    pos: Position,
) -> Option<ProfileKey> {
    let parsed = &doc.parsed;
    let idx = parsed.find_token_at_cursor(pos.line, pos.character)?;
    let token = &parsed.tokens[idx];
    if !matches!(token.kind, cogitator_parser::TokenKind::Word(_)) {
        return None;
    }
    let word = parsed.literal(token).trim_matches(['"', '`']);
    column_named(ctx, model, lineage, word)
}

/// `name` as an output column of the model (lineage or YAML), spelled as
/// the model spells it; `None` when the model has no such column.
pub fn column_named(
    ctx: &DbtContext,
    model: &str,
    lineage: Option<&ModelLineage>,
    name: &str,
) -> Option<ProfileKey> {
    let produced = lineage
        .into_iter()
        .flat_map(|l| l.columns.iter().map(|c| c.name.as_str()));
    let documented = ctx
        .model_detail_map
        .get(model)
        .into_iter()
        .flat_map(|d| d.columns.iter().map(|c| c.name.as_str()));
    produced
        .chain(documented)
        .find(|column| column.eq_ignore_ascii_case(name))
        .map(|column| ProfileKey::new(model, column))
}

pub const PROFILE_ACTION_PREFIX: &str = "Profile column";

/// "Profile column 'status'": runs dbt.profileColumn for the column under
/// the cursor (offered only while `warehouse.profile` is on).
pub fn profile_action(key: &ProfileKey, uri: &str, pos: Position) -> cogitator_lsp::CodeAction {
    cogitator_lsp::CodeAction::command_only(
        format!("{PROFILE_ACTION_PREFIX} '{}'", key.column),
        cogitator_lsp::Command {
            title: format!("{PROFILE_ACTION_PREFIX} '{}'", key.column),
            command: "dbt.profileColumn".to_string(),
            arguments: vec![serde_json::json!({"uri": uri, "position": pos})],
        },
    )
    .at_cursor()
}

/// What the profile query answered.
#[derive(Debug, Clone, PartialEq)]
pub struct Figures {
    pub rows: u64,
    pub distinct: u64,
    pub nulls: u64,
    pub min: Option<String>,
    pub max: Option<String>,
    pub top: Vec<TopValue>,
}

/// The tagged rows of [`profile_sql`] → the figures. Column names match
/// case-insensitively (Snowflake answers upper case); numbers may come
/// as JSON numbers or text.
pub fn read_rows(columns: &[String], rows: &[Vec<Value>]) -> Result<Figures, Guidance> {
    let at = |name: &str| columns.iter().position(|c| c.eq_ignore_ascii_case(name));
    let (Some(kind), Some(value), Some(value2), Some(n), Some(distinct), Some(nulls)) = (
        at("kind"),
        at("value"),
        at("value2"),
        at("n"),
        at("distinct_count"),
        at("null_count"),
    ) else {
        return Err(
            Guidance::new("the profile query answered unexpected columns.")
                .because(format!("got {}", columns.join(", ")))
                .fix("report this with the adapter you use."),
        );
    };
    let text = |v: &Value| match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    };
    let number = |v: &Value| match v {
        Value::Number(n) => n.as_u64().or_else(|| n.as_f64().map(|f| f as u64)),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    };
    let mut stats = None;
    let mut top = Vec::new();
    for row in rows {
        let cell = |i: usize| row.get(i).unwrap_or(&Value::Null);
        match text(cell(kind)).as_deref() {
            Some("stats") => {
                stats = Some((
                    number(cell(n)).unwrap_or(0),
                    number(cell(distinct)).unwrap_or(0),
                    number(cell(nulls)).unwrap_or(0),
                    text(cell(value)),
                    text(cell(value2)),
                ))
            }
            Some("top") => top.push(TopValue {
                value: text(cell(value)).unwrap_or_default(),
                count: number(cell(n)).unwrap_or(0),
            }),
            _ => {}
        }
    }
    let Some((rows, distinct, nulls, min, max)) = stats else {
        return Err(
            Guidance::new("the profile query returned no statistics row.")
                .fix("check that the model's relation exists: run dbt build first."),
        );
    };
    top.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.value.cmp(&b.value)));
    top.truncate(TOP_VALUES);
    Ok(Figures {
        rows,
        distinct,
        nulls,
        min,
        max,
        top,
    })
}

/// Assembles a profile from the query's answer.
pub fn profile_of(
    key: &ProfileKey,
    relation: &str,
    engine: &str,
    stamp: &str,
    sql: String,
    columns: &[String],
    rows: &[Vec<Value>],
) -> Result<ColumnProfile, Guidance> {
    let Figures {
        rows: count,
        distinct,
        nulls,
        min,
        max,
        top,
    } = read_rows(columns, rows)?;
    Ok(ColumnProfile {
        model: key.model.clone(),
        column: key.column.clone(),
        relation: relation.to_string(),
        rows: count,
        distinct,
        nulls,
        null_rate: if count == 0 {
            0.0
        } else {
            nulls as f64 / count as f64
        },
        min,
        max,
        top,
        engine: engine.to_string(),
        stamp: stamp.to_string(),
        sql,
    })
}

#[cfg(test)]
mod tests;
