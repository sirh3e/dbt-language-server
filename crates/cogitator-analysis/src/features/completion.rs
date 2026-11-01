//! textDocument/completion + completionItem/resolve (03 §9.4, P2 resolve).

use std::sync::LazyLock;

use cogitator_lsp::{completion_kind, CompletionItem, Position};
use cogitator_parser::TokenKind;
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
        write_suggestions(ctx, doc, pos)
    }
}

/// Plain-SQL write assist: dialect functions + SQL keywords + the document's
/// own CTEs/aliases/identifiers + documented columns of every relation the
/// document refs or sources (FEATURE_REQUESTS "writing suggestions").
fn write_suggestions(ctx: &DbtContext, doc: &Document, pos: Position) -> Vec<CompletionItem> {
    let mut items = ctx.dialect.function_completion_items();
    items.extend(keyword_items());
    items.extend(local_identifier_items(doc, pos));
    items.extend(relation_column_items(ctx, doc));
    items
}

/// SQL keywords from sqlparser's global table (covers the Snowflake set —
/// QUALIFY, ILIKE, …) as lowercase suggestions.
fn keyword_items() -> impl Iterator<Item = CompletionItem> {
    sqlparser::keywords::ALL_KEYWORDS.iter().map(|kw| {
        let word = kw.to_lowercase();
        CompletionItem {
            label: word.clone(),
            detail: "SQL keyword".to_string(),
            documentation: String::new(),
            kind: completion_kind::KEYWORD,
            insert_text: word.clone(),
            sort_text: format!("z{word}"), // keywords sort after everything local
            data: None,
        }
    })
}

/// CTE names and every identifier already written in the document (table
/// aliases, column names in use) — excluding the word under the cursor.
fn local_identifier_items(doc: &Document, pos: Position) -> Vec<CompletionItem> {
    use cogitator_parser::Keyword;

    let under_cursor = doc
        .parsed
        .find_token_at_cursor(pos.line, pos.character.saturating_sub(1));
    let mut seen = std::collections::BTreeSet::new();
    let mut items = Vec::new();

    for name in doc.parsed.def_tokens.keys() {
        if seen.insert(name.to_string()) {
            items.push(CompletionItem {
                label: name.to_string(),
                detail: "CTE in this file".to_string(),
                documentation: String::new(),
                kind: completion_kind::VARIABLE,
                insert_text: name.to_string(),
                sort_text: format!("a{name}"),
                data: None,
            });
        }
    }
    for (i, token) in doc.parsed.tokens.iter().enumerate() {
        if !matches!(
            token.kind,
            TokenKind::Word(Keyword::NoKeyword) | TokenKind::Ident
        ) {
            continue;
        }
        if under_cursor == Some(i) {
            continue;
        }
        let literal = doc.parsed.literal(token).to_string();
        if seen.insert(literal.clone()) {
            items.push(CompletionItem {
                label: literal.clone(),
                detail: "identifier in this file".to_string(),
                documentation: String::new(),
                kind: completion_kind::TEXT,
                insert_text: literal.clone(),
                sort_text: format!("b{literal}"),
                data: None,
            });
        }
    }
    items
}

/// Documented columns of every model this document refs and every source
/// table it reads, straight from the properties YAML.
fn relation_column_items(ctx: &DbtContext, doc: &Document) -> Vec<CompletionItem> {
    let mut seen = std::collections::BTreeSet::new();
    let mut items = Vec::new();
    let mut add = |relation: &str, column: &cogitator_schemas::ColumnEntry| {
        if seen.insert((relation.to_string(), column.name.clone())) {
            items.push(CompletionItem {
                label: column.name.clone(),
                detail: format!("Column of {relation}"),
                documentation: column.description.clone(),
                kind: completion_kind::FIELD,
                insert_text: column.name.clone(),
                sort_text: format!("a{}", column.name),
                data: None,
            });
        }
    };

    for token in &doc.parsed.tokens {
        match token.kind {
            TokenKind::Ref => {
                let name = doc.parsed.literal(token);
                if let Some(model) = ctx.model_detail_map.get(name) {
                    for column in &model.columns {
                        add(name, column);
                    }
                }
            }
            TokenKind::SourceTable => {
                let table_name = doc.parsed.literal(token);
                for source in ctx.source_detail_map.values() {
                    if let Some(table) = source.tables.get(table_name) {
                        for column in &table.columns {
                            add(table_name, column);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    items
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
