//! Incremental strategies, both ways round:
//!
//! - **dbt.incrementalSql**: the statements an incremental run executes
//!   for the model's strategy (`append`, `merge`, `delete+insert`,
//!   `insert_overwrite`, `microbatch`; the config's, an override, or the
//!   adapter's default) — the new rows staged from the model rendered
//!   with `is_incremental()` true, then the strategy's DML in the shape
//!   dbt-core's and the adapters' macros give it, with `unique_key`,
//!   `incremental_predicates`, `merge_update_columns`,
//!   `merge_exclude_columns`, `partition_by` and `event_time` applied —
//!   and the full refresh beside them.
//! - **templates**: on a model that is not incremental, a code action per
//!   strategy that makes it one (its config and the `is_incremental()`
//!   filter, with a unique key and a timestamp guessed from the
//!   documented columns); on an incremental model, one per other
//!   strategy that switches its config.
//!
//! Pure: the shell assembles the [`PreviewInputs`].

use cogitator_error::guidance::Guidance;
use cogitator_lsp::{
    CodeAction, Command, DocumentChange, Position, Range, TextDocumentEdit, TextEdit, WorkspaceEdit,
};
use cogitator_parser::LineTable;
use serde::Serialize;
use serde_json::Value as Json;

use super::jinja_preview::{expand_jinja, PreviewInputs};
use super::model_checks::config::{calls, Call};
use super::{model_name_from_uri, DbtContext, Document};

/// The strategies, in the order they are offered.
pub const STRATEGIES: &[&str] = &[
    "append",
    "merge",
    "delete+insert",
    "insert_overwrite",
    "microbatch",
];

/// The dbt.incrementalSql answer.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncrementalSql {
    pub model: String,
    pub strategy: String,
    /// Why this strategy: `config`, `argument` or `adapter default`.
    pub strategy_source: String,
    pub adapter: String,
    pub unique_key: Vec<String>,
    /// What an incremental run executes, in order.
    pub steps: Vec<Step>,
    /// What the first run and `--full-refresh` execute.
    pub full_refresh: String,
    /// What the rendering could not know.
    pub notes: Vec<String>,
    /// Everything above as one SQL text to show.
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct Step {
    pub title: String,
    pub sql: String,
}

/// The model's incremental config, as its own SQL writes it.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct IncrementalConfig {
    pub strategy: Option<String>,
    pub unique_key: Vec<String>,
    pub predicates: Vec<String>,
    pub update_columns: Option<Vec<String>>,
    pub exclude_columns: Vec<String>,
    pub partition_field: Option<String>,
    pub event_time: Option<String>,
    pub batch_size: Option<String>,
}

impl IncrementalConfig {
    pub(crate) fn of(text: &str) -> Self {
        let mut config = IncrementalConfig::default();
        for (key, value) in crate::util::sql_config(text) {
            match key.as_str() {
                "incremental_strategy" => config.strategy = value.as_str().map(str::to_string),
                "unique_key" => config.unique_key = strings(&value),
                "incremental_predicates" => config.predicates = strings(&value),
                "merge_update_columns" => config.update_columns = Some(strings(&value)),
                "merge_exclude_columns" => config.exclude_columns = strings(&value),
                "partition_by" => {
                    config.partition_field = match &value {
                        Json::Object(map) => {
                            map.get("field").and_then(Json::as_str).map(str::to_string)
                        }
                        Json::String(s) => Some(s.clone()),
                        Json::Array(a) => a.first().and_then(Json::as_str).map(str::to_string),
                        _ => None,
                    }
                }
                "event_time" => config.event_time = value.as_str().map(str::to_string),
                "batch_size" => config.batch_size = value.as_str().map(str::to_string),
                _ => {}
            }
        }
        config
    }
}

fn strings(value: &Json) -> Vec<String> {
    match value {
        Json::String(s) => s
            .split(',')
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty())
            .collect(),
        Json::Array(values) => values
            .iter()
            .filter_map(Json::as_str)
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

/// The strategy an adapter takes when the config names none.
pub(crate) fn default_strategy(adapter: &str, has_unique_key: bool) -> &'static str {
    match adapter {
        "snowflake" | "bigquery" | "databricks" => "merge",
        "spark" => "append",
        _ if has_unique_key => "delete+insert",
        _ => "append",
    }
}

/// What an incremental run of `model` executes.
pub fn incremental_sql(
    model: &str,
    text: &str,
    adapter: &str,
    columns: &[String],
    inputs: &mut PreviewInputs,
    strategy: Option<&str>,
) -> Result<IncrementalSql, Guidance> {
    if let Some(s) = strategy.filter(|s| !STRATEGIES.contains(s)) {
        return Err(
            Guidance::new(format!("'{s}' is not an incremental strategy."))
                .suggest(cogitator_error::guidance::closest_matches(
                    s,
                    STRATEGIES.iter().copied(),
                ))
                .fix(format!("pass one of {}", STRATEGIES.join(", "))),
        );
    }
    let config = IncrementalConfig::of(text);
    let (strategy, source) = match (strategy, &config.strategy) {
        (Some(s), _) => (s.to_string(), "argument"),
        (None, Some(s)) => (s.clone(), "config"),
        (None, None) => (
            default_strategy(adapter, !config.unique_key.is_empty()).to_string(),
            "adapter default",
        ),
    };
    let this = inputs
        .model_relations
        .get(model)
        .cloned()
        .unwrap_or_else(|| model.to_string());
    inputs.this = Some(this.clone());
    inputs.is_incremental = true;
    let new_rows = expand_jinja(text, inputs)?.sql;
    inputs.is_incremental = false;
    let full = expand_jinja(text, inputs)?.sql;

    let tmp = format!("{this}__dbt_tmp");
    let mut notes = Vec::new();
    if columns.is_empty() {
        notes.push(
            "the model documents no columns, so `*` and `<column>` stand for the columns dbt \
             reads from the table"
                .to_string(),
        );
    }
    let stage = Step {
        title: "stage the new rows (the model with is_incremental() true)".to_string(),
        sql: format!(
            "create temporary table {tmp} as (\n{}\n);\n",
            indent(&new_rows)
        ),
    };
    let plan = Plan {
        this: &this,
        tmp: &tmp,
        columns,
        config: &config,
        adapter,
    };
    let mut steps = vec![stage];
    match strategy.as_str() {
        "append" => steps.push(plan.append()),
        "delete+insert" => {
            if config.unique_key.is_empty() {
                return Err(Guidance::new("delete+insert needs a unique_key.")
                    .because("dbt deletes the rows of the target whose key the new rows have")
                    .fix("add unique_key='<column>' to the model's config"));
            }
            steps.push(plan.delete_by_key());
            steps.push(plan.append());
        }
        "merge" => {
            if config.unique_key.is_empty() {
                notes.push("without a unique_key a merge only inserts (`on (false)`)".to_string());
            }
            steps.push(plan.merge());
        }
        "insert_overwrite" => {
            let Some(field) = config.partition_field.as_deref() else {
                return Err(Guidance::new("insert_overwrite needs a partition_by.")
                    .because("dbt replaces the partitions the new rows fall in")
                    .fix(
                        "add partition_by={'field': '<column>', 'data_type': 'date'} to the config",
                    ));
            };
            steps.extend(plan.overwrite(field, &mut notes));
        }
        "microbatch" => {
            let Some(event_time) = config.event_time.as_deref() else {
                return Err(Guidance::new("microbatch needs an event_time.")
                    .because("dbt splits the run into batches of the event_time column")
                    .fix("add event_time='<timestamp column>', batch_size='day' and begin='<first day>'"));
            };
            let batch = config.batch_size.as_deref().unwrap_or("day");
            steps[0].title = format!(
                "for each {batch} batch [<batch start>, <batch end>): stage its rows — dbt filters \
                 every ref and source that has an event_time to the batch"
            );
            steps.push(plan.delete_batch(event_time));
            steps.push(plan.append());
            notes.push(format!(
                "dbt runs the steps once per {batch} from `begin` (or the lookback) to now; \
                 `is_incremental()` filters in the model are not needed"
            ));
        }
        other => {
            notes.push(format!(
                "'{other}' is a custom strategy: dbt runs the macro get_incremental_{other}_sql"
            ));
        }
    }
    let full_refresh = format!("create table {this} as (\n{}\n);\n", indent(&full));
    let mut answer = IncrementalSql {
        model: model.to_string(),
        strategy,
        strategy_source: source.to_string(),
        adapter: adapter.to_string(),
        unique_key: config.unique_key.clone(),
        steps,
        full_refresh,
        notes,
        text: String::new(),
    };
    answer.text = shown_text(&answer);
    Ok(answer)
}

struct Plan<'a> {
    this: &'a str,
    tmp: &'a str,
    columns: &'a [String],
    config: &'a IncrementalConfig,
    adapter: &'a str,
}

impl Plan<'_> {
    fn column_list(&self) -> Option<String> {
        (!self.columns.is_empty()).then(|| self.columns.join(", "))
    }

    fn append(&self) -> Step {
        let sql = match self.column_list() {
            Some(cols) => format!(
                "insert into {} ({cols})\n(\n    select {cols}\n    from {}\n);\n",
                self.this, self.tmp
            ),
            None => format!(
                "insert into {}\n(\n    select *\n    from {}\n);\n",
                self.this, self.tmp
            ),
        };
        Step {
            title: "insert the new rows".to_string(),
            sql,
        }
    }

    fn predicates(&self, joiner: &str) -> String {
        self.config
            .predicates
            .iter()
            .map(|p| format!("{joiner}{p}"))
            .collect()
    }

    fn delete_by_key(&self) -> Step {
        let keys = &self.config.unique_key;
        let sql = if keys.len() == 1 {
            let k = &keys[0];
            format!(
                "delete from {this}\nwhere ({k}) in (\n    select ({k})\n    from {tmp}\n){preds};\n",
                this = self.this,
                tmp = self.tmp,
                preds = self.predicates("\n    and "),
            )
        } else {
            let on = keys
                .iter()
                .map(|k| format!("{tmp}.{k} = {this}.{k}", tmp = self.tmp, this = self.this))
                .collect::<Vec<_>>()
                .join("\n    and ");
            format!(
                "delete from {this}\nusing {tmp}\nwhere (\n    {on}\n){preds};\n",
                this = self.this,
                tmp = self.tmp,
                preds = self.predicates("\n    and "),
            )
        };
        Step {
            title: format!(
                "delete the rows the new ones replace (by {})",
                keys.join(", ")
            ),
            sql,
        }
    }

    fn merge(&self) -> Step {
        let on = if self.config.unique_key.is_empty() {
            "(false)".to_string()
        } else {
            self.config
                .unique_key
                .iter()
                .map(|k| format!("DBT_INTERNAL_SOURCE.{k} = DBT_INTERNAL_DEST.{k}"))
                .collect::<Vec<_>>()
                .join("\n    and ")
        };
        let preds = self.predicates("\n    and ");
        let updated: Vec<&String> = match &self.config.update_columns {
            Some(cols) => cols.iter().collect(),
            None => self
                .columns
                .iter()
                .filter(|c| !self.config.exclude_columns.contains(c))
                .collect(),
        };
        let update = if updated.is_empty() {
            "    <column> = DBT_INTERNAL_SOURCE.<column>  -- each column".to_string()
        } else {
            updated
                .iter()
                .map(|c| format!("    {c} = DBT_INTERNAL_SOURCE.{c}"))
                .collect::<Vec<_>>()
                .join(",\n")
        };
        let (cols, values) = match self.column_list() {
            Some(cols) => {
                let values = self
                    .columns
                    .iter()
                    .map(|c| format!("DBT_INTERNAL_SOURCE.{c}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                (cols, values)
            }
            None => (
                "<column>, …".to_string(),
                "DBT_INTERNAL_SOURCE.<column>, …".to_string(),
            ),
        };
        let matched = if self.config.unique_key.is_empty() {
            String::new()
        } else {
            format!("when matched then update set\n{update}\n")
        };
        Step {
            title: "merge the new rows into the table".to_string(),
            sql: format!(
                "merge into {this} as DBT_INTERNAL_DEST\nusing {tmp} as DBT_INTERNAL_SOURCE\n\
                 on {on}{preds}\n{matched}when not matched then insert\n    ({cols})\nvalues\n    ({values});\n",
                this = self.this,
                tmp = self.tmp,
            ),
        }
    }

    fn overwrite(&self, field: &str, notes: &mut Vec<String>) -> Vec<Step> {
        let (this, tmp) = (self.this, self.tmp);
        match self.adapter {
            "bigquery" => vec![Step {
                title: format!("replace the partitions of {field} the new rows fall in"),
                sql: format!(
                    "merge into {this} as DBT_INTERNAL_DEST\nusing {tmp} as DBT_INTERNAL_SOURCE\n\
                     on false\nwhen not matched by source\n    and DBT_INTERNAL_DEST.{field} in (\n        \
                     select distinct {field}\n        from {tmp}\n    )\n    then delete\n\
                     when not matched then insert row;\n"
                ),
            }],
            "spark" | "databricks" => vec![Step {
                title: format!("overwrite the partitions of {field} the new rows fall in"),
                sql: format!("insert overwrite table {this}\npartition ({field})\nselect * from {tmp};\n"),
            }],
            _ => {
                notes.push(format!(
                    "{} has no insert_overwrite of its own; this is the shape of the adapters that \
                     have one, as a delete and an insert",
                    self.adapter
                ));
                vec![
                    Step {
                        title: format!("delete the partitions of {field} the new rows fall in"),
                        sql: format!(
                            "delete from {this}\nwhere {field} in (\n    select distinct {field}\n    from {tmp}\n);\n"
                        ),
                    },
                    self.append(),
                ]
            }
        }
    }

    fn delete_batch(&self, event_time: &str) -> Step {
        Step {
            title: "delete the batch's rows from the table".to_string(),
            sql: format!(
                "delete from {}\nwhere {event_time} >= '<batch start>'\n    and {event_time} < '<batch end>';\n",
                self.this
            ),
        }
    }
}

fn indent(sql: &str) -> String {
    sql.trim_end()
        .lines()
        .map(|l| {
            if l.trim().is_empty() {
                String::new()
            } else {
                format!("    {l}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The text the code action opens.
pub fn shown_text(answer: &IncrementalSql) -> String {
    let key = if answer.unique_key.is_empty() {
        String::new()
    } else {
        format!(", unique_key {}", answer.unique_key.join(", "))
    };
    let mut out = format!(
        "-- {}: an incremental run with the {} strategy ({}{key}), on {}.\n",
        answer.model, answer.strategy, answer.strategy_source, answer.adapter
    );
    for note in &answer.notes {
        out.push_str(&format!("-- note: {note}\n"));
    }
    for (i, step) in answer.steps.iter().enumerate() {
        out.push_str(&format!("\n-- {}. {}\n{}", i + 1, step.title, step.sql));
    }
    out.push_str(&format!(
        "\n-- the first run, and every --full-refresh\n{}",
        answer.full_refresh
    ));
    out
}

// ---------------------------------------------------------------------------
// templates
// ---------------------------------------------------------------------------

/// The columns a template names, guessed from the documented ones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Guesses {
    pub key: String,
    pub timestamp: String,
    pub day: String,
}

pub(crate) fn guesses(ctx: &DbtContext, model: &str) -> Guesses {
    let columns = ctx
        .model_detail_map
        .get(model)
        .map(|d| d.columns.as_slice())
        .unwrap_or_default();
    let names: Vec<&str> = columns.iter().map(|c| c.name.as_str()).collect();
    let find =
        |pred: &dyn Fn(&str) -> bool| names.iter().copied().find(|n| pred(n)).map(str::to_string);
    let key = columns
        .iter()
        .find(|c| c.tests.iter().any(|t| t.name == "unique"))
        .map(|c| c.name.clone())
        .or_else(|| find(&|n| n == "id"))
        .or_else(|| find(&|n| n.ends_with("_id")))
        .unwrap_or_else(|| "id".to_string());
    let timestamp = find(&|n| n.contains("updated_at"))
        .or_else(|| find(&|n| n.ends_with("_at") || n.contains("timestamp") || n.ends_with("_ts")))
        .unwrap_or_else(|| "updated_at".to_string());
    let day = find(&|n| n.contains("date") || n.ends_with("_day") || n == "day")
        .unwrap_or_else(|| timestamp.clone());
    Guesses {
        key,
        timestamp,
        day,
    }
}

/// The config arguments a strategy needs, as Jinja keyword arguments.
pub(crate) fn strategy_arguments(strategy: &str, g: &Guesses) -> Vec<(String, String)> {
    let s = |v: &str| format!("'{v}'");
    let mut out = vec![
        ("materialized".to_string(), s("incremental")),
        ("incremental_strategy".to_string(), s(strategy)),
    ];
    match strategy {
        "merge" | "delete+insert" => out.push(("unique_key".to_string(), s(&g.key))),
        "insert_overwrite" => out.push((
            "partition_by".to_string(),
            format!("{{'field': '{}', 'data_type': 'date'}}", g.day),
        )),
        "microbatch" => {
            out.push(("event_time".to_string(), s(&g.timestamp)));
            out.push(("batch_size".to_string(), s("day")));
            out.push(("begin".to_string(), s("2024-01-01")));
        }
        _ => {}
    }
    out
}

/// The keys a strategy switch replaces in an existing config.
const STRATEGY_KEYS: &[&str] = &[
    "materialized",
    "incremental_strategy",
    "unique_key",
    "partition_by",
    "event_time",
    "batch_size",
    "begin",
];

/// `{{ config(...) }}`'s new text: the strategy's arguments, then every
/// other argument of the existing call as it was written.
fn config_text(text: &str, existing: Option<&Call>, arguments: &[(String, String)]) -> String {
    let mut parts: Vec<String> = arguments.iter().map(|(k, v)| format!("{k}={v}")).collect();
    if let Some(call) = existing {
        for kwarg in &call.kwargs {
            if !STRATEGY_KEYS.contains(&kwarg.key.as_str()) {
                parts.push(text[kwarg.key_span.0..kwarg.value_span.1].to_string());
            }
        }
    }
    format!("config(\n    {}\n)", parts.join(",\n    "))
}

/// The `is_incremental()` block a strategy filters the new rows with.
fn filter_block(strategy: &str, g: &Guesses, sql: &str) -> Option<String> {
    let column = match strategy {
        "microbatch" => return None,
        "insert_overwrite" => &g.day,
        _ => &g.timestamp,
    };
    // the last statement's own `where` decides between where and and
    let tail = sql
        .rsplit_once(" from ")
        .map_or(sql, |(_, t)| t)
        .to_ascii_lowercase();
    let word = if tail.contains("where ") {
        "and"
    } else {
        "where"
    };
    Some(format!(
        "\n{{% if is_incremental() %}}\n    -- only the rows newer than the newest already built\n    \
         {word} {column} > (select max({column}) from {{{{ this }}}})\n{{% endif %}}\n"
    ))
}

/// The template actions of a model: "Make incremental" per strategy on
/// a model that is not incremental, "Switch to" per other strategy on
/// one that is.
pub fn incremental_template_actions(
    ctx: &DbtContext,
    doc: &Document,
    uri: &str,
) -> Vec<CodeAction> {
    let Some(model) = model_name_from_uri(uri).filter(|m| ctx.model_detail_map.contains_key(m))
    else {
        return Vec::new();
    };
    let text: &str = &doc.text;
    let found = calls(text);
    let config = IncrementalConfig::of(text);
    let incremental = super::is_incremental_model(text, None);
    let current = config.strategy.clone().unwrap_or_else(|| {
        default_strategy(ctx.dialect.as_str(), !config.unique_key.is_empty()).to_string()
    });
    let g = guesses(ctx, &model);
    let table = LineTable::new(text);
    let position = |offset: usize| {
        let (line, character) = table.line_col(offset.min(text.len()) as u32);
        Position { line, character }
    };
    STRATEGIES
        .iter()
        .filter(|s| !incremental || **s != current)
        .map(|strategy| {
            let arguments = strategy_arguments(strategy, &g);
            let new_config = config_text(text, found.first(), &arguments);
            let mut edits = Vec::new();
            match found.first() {
                Some(call) => edits.push(TextEdit {
                    range: Range {
                        start: position(call.span.0),
                        end: position(call.span.1),
                    },
                    new_text: new_config,
                }),
                None => edits.push(TextEdit {
                    range: Range::zero_width(position(0)),
                    new_text: format!("{{{{ {new_config} }}}}\n\n"),
                }),
            }
            if !incremental {
                if let Some(block) = filter_block(strategy, &g, text) {
                    let end = text.trim_end().len();
                    edits.push(TextEdit {
                        range: Range {
                            start: position(end),
                            end: position(text.len()),
                        },
                        new_text: block,
                    });
                }
            }
            let detail = arguments
                .iter()
                .skip(2)
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join(", ");
            let verb = if incremental {
                "Switch to the"
            } else {
                "Make incremental with the"
            };
            let title = if detail.is_empty() {
                format!("{verb} {strategy} strategy")
            } else {
                format!("{verb} {strategy} strategy ({detail})")
            };
            CodeAction {
                title,
                kind: Some("refactor.rewrite"),
                diagnostics: None,
                edit: Some(WorkspaceEdit {
                    document_changes: vec![DocumentChange::Edit(TextDocumentEdit::new(
                        uri.to_string(),
                        edits,
                    ))],
                }),
                command: None,
                is_preferred: None,
                data: None,
            }
        })
        .collect()
}

/// "Show the SQL of an incremental run" on an incremental model: the
/// command opens it as a document (`show: true`) in every client.
pub fn incremental_sql_action(ctx: &DbtContext, doc: &Document, uri: &str) -> Option<CodeAction> {
    model_name_from_uri(uri).filter(|m| ctx.model_detail_map.contains_key(m))?;
    super::is_incremental_model(&doc.text, None).then(|| {
        let title = "Show the SQL of an incremental run".to_string();
        CodeAction::command_only(
            title.clone(),
            Command {
                title,
                command: "dbt.incrementalSql".to_string(),
                arguments: vec![serde_json::json!({"uri": uri, "show": true})],
            },
        )
    })
}

#[cfg(test)]
#[path = "incremental_sql_tests.rs"]
mod tests;
