//! textDocument/completion + completionItem/resolve (03 §9.4, P2 resolve).

use std::sync::LazyLock;

use dbt_ls_lsp::{completion_kind, CompletionItem, Position};
use regex::Regex;

use super::{display_value, DbtContext, Document};

// Q1 fixed: `[a-zA-Z0-9_]` instead of Go's `[a-zA-z]` typo.
static REF_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\bref\(('|")[a-zA-Z0-9_]*$"#).unwrap());

static SOURCE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\bsource\(('|")[a-zA-Z0-9_]*$"#).unwrap());

static VAR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\bvar\(('|")[a-zA-Z0-9_]*$"#).unwrap());

static JINJA_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{\{\s*").unwrap());

pub fn completion(ctx: &DbtContext, doc: &Document, pos: Position) -> Vec<CompletionItem> {
    let lines: Vec<&str> = doc.text.split('\n').collect();
    let Some(&line) = lines.get(pos.line as usize) else {
        return Vec::new(); // Go panicked here (Q8-adjacent); degrade instead
    };
    let cursor = (pos.character as usize).min(line.len());
    let cursor = (0..=cursor)
        .rev()
        .find(|&i| line.is_char_boundary(i))
        .unwrap_or(0);
    let before = &line[..cursor];
    let after = &line[cursor..];

    if REF_RE.is_match(before) {
        ref_items(ctx, &suffix_for(line, after, "ref"))
    } else if SOURCE_RE.is_match(before) {
        source_items(ctx, &suffix_for(line, after, "source"), &quote_type(before))
    } else if VAR_RE.is_match(before) {
        var_items(ctx, &suffix_for(line, after, "var"))
    } else if JINJA_RE.is_match(before) {
        macro_items(ctx)
    } else {
        ctx.dialect.function_completion_items()
    }
}

fn ref_items(ctx: &DbtContext, suffix: &str) -> Vec<CompletionItem> {
    ctx.model_detail_map
        .iter()
        .map(|(name, m)| CompletionItem {
            label: name.clone(),
            detail: format!("Project: {}", m.project_name),
            documentation: String::new(), // resolved lazily (completionItem/resolve)
            kind: completion_kind::REFERENCE,
            insert_text: format!("{name}{suffix}"),
            sort_text: name.clone(),
            data: Some(serde_json::json!({"kind": "model", "name": name})),
        })
        .collect()
}

fn source_items(ctx: &DbtContext, suffix: &str, quote: &str) -> Vec<CompletionItem> {
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
                    serde_json::json!({"kind": "source", "name": source.name, "table": table.name}),
                ),
            })
        })
        .collect()
}

fn var_items(ctx: &DbtContext, suffix: &str) -> Vec<CompletionItem> {
    ctx.variable_detail_map
        .keys()
        .map(|name| CompletionItem {
            label: name.clone(),
            detail: name.clone(),
            documentation: String::new(),
            kind: completion_kind::VARIABLE,
            insert_text: format!("{name}{suffix}"),
            sort_text: name.clone(),
            data: Some(serde_json::json!({"kind": "variable", "name": name})),
        })
        .collect()
}

fn macro_items(ctx: &DbtContext) -> Vec<CompletionItem> {
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
                data: Some(serde_json::json!({
                    "kind": "macro", "name": m.name, "package": m.project_name,
                })),
            }
        })
        .collect()
}

/// Closing-suffix mirroring (03 §9.4.1): `{{ ref('` → `') }}`.
fn suffix_for(line: &str, after: &str, kind: &str) -> String {
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
fn quote_type(before: &str) -> String {
    before
        .chars()
        .rev()
        .find(|c| *c == '\'' || *c == '"')
        .map(String::from)
        .unwrap_or_default()
}

/// completionItem/resolve (P2): fill documentation lazily from the maps.
pub fn resolve_completion_documentation(
    ctx: &DbtContext,
    data: &serde_json::Value,
) -> Option<String> {
    let kind = data.get("kind")?.as_str()?;
    let name = data.get("name")?.as_str()?;
    match kind {
        "model" => ctx
            .model_detail_map
            .get(name)
            .map(|m| m.description.clone()),
        "source" => {
            let table = data.get("table")?.as_str()?;
            let source = ctx.source_detail_map.get(name)?;
            let t = source.tables.get(table)?;
            Some(format!(
                "{}\n\nTable: {}\n{}",
                source.description, t.name, t.description
            ))
        }
        "variable" => ctx
            .variable_detail_map
            .get(name)
            .map(|v| display_value(&v.value)),
        "macro" => {
            let package = data
                .get("package")
                .and_then(|p| p.as_str())
                .unwrap_or(&ctx.project.name);
            ctx.macro_detail_map
                .get(package)?
                .get(name)
                .map(|m| m.description.clone())
        }
        "function" => ctx.dialect.function_docs().get(name).cloned(),
        _ => None,
    }
}
