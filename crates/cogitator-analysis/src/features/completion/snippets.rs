//! Model-aware snippets: an `{% if is_incremental() %}` block filtered on
//! the model's own growing column, and an incremental `config()` with its
//! real merge key. Pure over the context and what the shell knows of the
//! model's output columns.
//!
//! The column, in order: the model's `event_time` (YAML or manifest); an
//! upstream's declared `event_time` / `loaded_at_field` the model passes
//! through (the incrementality analysis's declared delta input); a
//! temporal output column named like an event time
//! (`looks_like_event_time`). Only the last is a guess, and a guess is a
//! tab stop (`${1:created_at}`); nothing at all is `${1:updated_at}`.
//! The merge key: a `primary_key` constraint, else a single-column
//! unique test or key, else a composite key, else a tab stop.

use cogitator_lsp::{completion_kind, insert_text_format, CompletionItem, LabelDetails};
use cogitator_rules::canon::incrementality::looks_like_event_time;
use cogitator_rules::rules::incr::watermark::newer_than_built;

use super::super::column_types::ModelColumnTypes;
use super::{DbtContext, Document};
use crate::graph::NodeKind;

/// What the shell knows of the model beyond the context.
#[derive(Debug, Clone, Default)]
pub struct ModelHints {
    /// The model's inferred output columns (the schema snapshot's).
    pub outputs: Option<ModelColumnTypes>,
    /// `config.event_time` from the manifest.
    pub manifest_event_time: Option<String>,
}

/// Where the column came from: a declared one is written as it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Column {
    Declared(String),
    Guessed(String),
    Unknown,
}

impl Column {
    fn snippet(&self) -> String {
        match self {
            Column::Declared(c) => escape(c),
            Column::Guessed(c) => format!("${{1:{}}}", escape(c)),
            Column::Unknown => "${1:updated_at}".to_string(),
        }
    }
}

fn escape(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('$', "\\$")
        .replace('}', "\\}")
}

/// The growing column of `model`, as the module docs order it.
pub(crate) fn incremental_column(
    ctx: &DbtContext,
    doc: &Document,
    word: std::ops::Range<usize>,
    model: &str,
    hints: &ModelHints,
) -> Column {
    let details = ctx.model_detail_map.get(model);
    if let Some(own) = details
        .and_then(|d| d.event_time.clone())
        .or_else(|| hints.manifest_event_time.clone())
    {
        return Column::Declared(own);
    }
    // inferred outputs, else the open text's own select list, else YAML
    let written: Vec<String> = match &hints.outputs {
        Some(_) => Vec::new(),
        None => written_columns(ctx, doc, word),
    };
    let outputs: Vec<&str> = match &hints.outputs {
        Some(types) => types.columns.iter().map(|c| c.name.as_str()).collect(),
        None if !written.is_empty() => written.iter().map(String::as_str).collect(),
        None => details
            .map(|d| d.columns.iter().map(|c| c.name.as_str()).collect())
            .unwrap_or_default(),
    };
    let produced = |name: &str| outputs.iter().any(|o| o.eq_ignore_ascii_case(name));
    for up in ctx.graph.upstream_of(model) {
        let declared: Vec<&str> = match up.kind {
            NodeKind::Model => ctx
                .model_detail_map
                .get(&up.name)
                .and_then(|d| d.event_time.as_deref())
                .into_iter()
                .collect(),
            NodeKind::Source => ctx
                .source_detail_map
                .get(&up.name)
                .map(|s| {
                    s.tables
                        .values()
                        .filter_map(|t| t.event_time.as_deref())
                        .collect()
                })
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        if let Some(column) = declared.into_iter().find(|c| produced(c)) {
            return Column::Declared(column.to_string());
        }
    }
    let temporal = |name: &str| {
        hints.outputs.as_ref().is_none_or(|t| {
            t.column(name).is_none_or(|c| {
                matches!(
                    c.sql_type,
                    cogitator_ir::SqlType::Date
                        | cogitator_ir::SqlType::Timestamp { .. }
                        | cogitator_ir::SqlType::Unknown
                )
            })
        })
    };
    outputs
        .iter()
        .find(|c| looks_like_event_time(c) && temporal(c))
        .map_or(Column::Unknown, |c| Column::Guessed(c.to_string()))
}

/// The open text's output column names, read without the word being
/// typed (which would not parse).
fn written_columns(ctx: &DbtContext, doc: &Document, word: std::ops::Range<usize>) -> Vec<String> {
    use cogitator_parser::{DocumentParser, JinjaSqlParser};
    let names = |parsed: &cogitator_parser::ParsedDocument| {
        crate::lineage::column_lineage(parsed, &ctx.dialect)
            .map(|l| l.columns.into_iter().map(|c| c.name).collect::<Vec<_>>())
    };
    if let Some(found) = names(&doc.parsed) {
        return found;
    }
    let mut text = doc.text.to_string();
    if word.end <= text.len()
        && text.is_char_boundary(word.start)
        && text.is_char_boundary(word.end)
    {
        text.replace_range(word, "");
    }
    JinjaSqlParser
        .parse(&text, &ctx.dialect)
        .ok()
        .and_then(|parsed| names(&parsed))
        .unwrap_or_default()
}

/// The merge key of `model`, quoted for `config(unique_key=…)`, or a tab
/// stop when nothing declares one.
pub(crate) fn merge_key(ctx: &DbtContext, model: &str) -> String {
    let Some(details) = ctx.model_detail_map.get(model) else {
        return "'${1:id}'".to_string();
    };
    let quote = |c: &str| format!("'{}'", escape(c));
    if let Some(pk) = details.columns.iter().find(|c| c.is_primary_key()) {
        return quote(&pk.name);
    }
    if let Some(one) = details.unique_keys.iter().find(|k| k.len() == 1) {
        return quote(&one[0]);
    }
    if let Some(col) = details.columns.iter().find(|c| c.proven_by("unique")) {
        return quote(&col.name);
    }
    if let Some(set) = details.unique_keys.first() {
        let list: Vec<String> = set.iter().map(|c| quote(c)).collect();
        return format!("[{}]", list.join(", "));
    }
    "'${1:id}'".to_string()
}

/// `and` when the statement before the cursor already filters.
fn joiner(doc: &Document, offset: usize) -> &'static str {
    let before = doc
        .text
        .get(..offset)
        .unwrap_or(&doc.text)
        .to_ascii_lowercase();
    match (before.rfind("where"), before.rfind("from")) {
        (Some(w), Some(f)) if w > f => "and",
        _ => "where",
    }
}

fn snippet_item(label: &str, detail: &str, docs: String, body: String) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        detail: detail.to_string(),
        documentation: docs,
        kind: completion_kind::SNIPPET,
        insert_text: body,
        sort_text: format!("0{label}"),
        insert_text_format: Some(insert_text_format::SNIPPET),
        label_details: Some(LabelDetails {
            detail: None,
            description: Some("snippet".to_string()),
        }),
        markdown: true,
        ..Default::default()
    }
}

/// The snippets for the word being typed (`before` the cursor), or for
/// a `{% if ` the cursor is in. Empty outside a model file.
pub(crate) fn snippet_items(
    ctx: &DbtContext,
    doc: &Document,
    model: &str,
    before: &str,
    after: &str,
    offset: usize,
    hints: &ModelHints,
) -> Vec<CompletionItem> {
    if !ctx.model_detail_map.contains_key(model) {
        return Vec::new();
    }
    let word: String = before
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let head = before[..before.len() - word.len()].trim_end();
    let typed = offset - word.len()..offset;
    let column = incremental_column(ctx, doc, typed, model, hints);
    let col = column.snippet();
    let filter = format!("{} {}", joiner(doc, offset), newer_than_built(&col, &col));
    let why = match &column {
        Column::Declared(c) => format!("Filters on `{c}`, the column declared to grow."),
        Column::Guessed(c) => format!("Filters on `{c}` (a guess from its name: check it)."),
        Column::Unknown => "No growing column is known: fill in the column.".to_string(),
    };
    if head.ends_with("{% if") || head.ends_with("{%- if") {
        if !"is_incremental".starts_with(&word.to_ascii_lowercase()) {
            return Vec::new();
        }
        let rest = after.trim_start();
        let body = if rest.starts_with("%}") || rest.starts_with("-%}") {
            "is_incremental()$0".to_string()
        } else {
            format!("is_incremental() %}}\n    {filter}\n{{% endif %}}$0")
        };
        return vec![snippet_item(
            "is_incremental()",
            "incremental filter",
            why,
            body,
        )];
    }
    if word.len() < 3
        || before.rfind("{{") > before.rfind("}}")
        || before.rfind("{%") > before.rfind("%}")
    {
        return Vec::new();
    }
    let word = word.to_ascii_lowercase();
    let mut out = Vec::new();
    if "is_incremental".starts_with(&word) {
        out.push(snippet_item(
            "is_incremental",
            "{% if is_incremental() %} block",
            why,
            format!("{{% if is_incremental() %}}\n    {filter}\n{{% endif %}}$0"),
        ));
    }
    if "config".starts_with(&word) {
        let key = merge_key(ctx, model);
        out.push(snippet_item(
            "config incremental",
            "{{ config(materialized='incremental', …) }}",
            "An incremental config with the model's merge key (from its primary key, unique tests or combination tests).".to_string(),
            format!("{{{{ config(materialized='incremental', unique_key={key}) }}}}$0"),
        ));
    }
    out
}
