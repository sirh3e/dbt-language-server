//! Completion providers for the dbt call positions: inside ref('…'),
//! source('…','…'), var('…') and the `{{` macro context.

use cogitator_lsp::{completion_kind, CompletionData, CompletionItem};
use regex::Regex;

use super::DbtContext;

pub fn ref_items(ctx: &DbtContext, suffix: &str) -> Vec<CompletionItem> {
    ctx.model_detail_map
        .iter()
        .map(|(name, m)| CompletionItem {
            label: name.clone(),
            detail: format!("Project: {}", m.project_name),
            documentation: String::new(), // resolved lazily (completionItem/resolve)
            kind: completion_kind::REFERENCE,
            insert_text: format!("{name}{suffix}"),
            sort_text: name.clone(),
            data: Some(CompletionData::Model { name: name.clone() }.into_value()),
        })
        .collect()
}

pub fn source_items(ctx: &DbtContext, suffix: &str, quote: &str) -> Vec<CompletionItem> {
    ctx.source_detail_map
        .iter()
        .flat_map(|(key, source)| {
            source.tables.values().map(move |table| CompletionItem {
                label: format!("{} - {}", source.name, table.name),
                detail: format!("Source: {}", source.name),
                documentation: String::new(),
                kind: completion_kind::REFERENCE,
                insert_text: format!("{}{quote}, {quote}{}{suffix}", source.name, table.name),
                sort_text: key.clone(),
                data: Some(
                    CompletionData::Source {
                        name: source.name.clone(),
                        table: table.name.clone(),
                    }
                    .into_value(),
                ),
            })
        })
        .collect()
}

pub fn var_items(ctx: &DbtContext, suffix: &str) -> Vec<CompletionItem> {
    ctx.variable_detail_map
        .keys()
        .map(|name| CompletionItem {
            label: name.clone(),
            detail: name.clone(),
            documentation: String::new(),
            kind: completion_kind::VARIABLE,
            insert_text: format!("{name}{suffix}"),
            sort_text: name.clone(),
            data: Some(CompletionData::Variable { name: name.clone() }.into_value()),
        })
        .collect()
}

pub fn doc_items(ctx: &DbtContext, suffix: &str) -> Vec<CompletionItem> {
    ctx.doc_detail_map
        .iter()
        .map(|(name, d)| CompletionItem {
            label: name.clone(),
            detail: format!("Project: {}", d.project_name),
            documentation: String::new(),
            kind: completion_kind::REFERENCE,
            insert_text: format!("{name}{suffix}"),
            sort_text: name.clone(),
            data: Some(CompletionData::Doc { name: name.clone() }.into_value()),
        })
        .collect()
}

pub fn macro_items(ctx: &DbtContext) -> Vec<CompletionItem> {
    ctx.macro_detail_map
        .values()
        .flat_map(|macros| macros.values())
        .map(|m| {
            let insert_text = if ctx.project.name == m.project_name {
                m.name.clone()
            } else {
                format!("{}.{}", m.project_name, m.name)
            };
            CompletionItem {
                label: m.name.clone(),
                detail: format!("Project: {}", m.project_name),
                documentation: String::new(),
                kind: completion_kind::SNIPPET,
                insert_text,
                sort_text: m.name.clone(),
                data: Some(
                    CompletionData::Macro {
                        name: m.name.clone(),
                        package: Some(m.project_name.clone()),
                    }
                    .into_value(),
                ),
            }
        })
        .collect()
}

/// Closing-suffix mirroring (03 §9.4.1): `{{ ref('` → `') }}`.
pub fn suffix_for(line: &str, after: &str, kind: &str) -> String {
    if !after.is_empty() {
        return String::new();
    }
    let re = Regex::new(&format!(r#"\{{\{{\s*{kind}\(('|")"#)).expect("static pattern");
    let Some(prefix) = re.find(line).map(|m| m.as_str()) else {
        return String::new();
    };
    let without_kind = prefix.replacen(kind, "", 1);
    without_kind
        .chars()
        .rev()
        .map(|c| match c {
            '(' => ')',
            '{' => '}',
            other => other,
        })
        .collect()
}

/// Quote style for the source item's middle quotes (03 §9.4.2; Q3 fixed:
/// genuinely the last quote before the cursor).
pub fn quote_type(before: &str) -> String {
    before
        .chars()
        .rev()
        .find(|c| *c == '\'' || *c == '"')
        .map(String::from)
        .unwrap_or_default()
}
