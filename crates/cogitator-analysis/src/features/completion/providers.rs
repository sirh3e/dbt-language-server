//! Completion item providers, composed per context with sort-text tiers:
//! `0` context-exact · `a` CTEs/locals/columns of refs · `b` buffer idents ·
//! `c` relations · `m` functions · `z` keywords.

use cogitator_lsp::{completion_kind, CompletionItem, Position};
use cogitator_parser::{Keyword, TokenKind};
use regex::Regex;

use super::scope::{cte_columns, RelationTarget, ScopeIndex};
use super::{DbtContext, Document};

fn item(
    label: impl Into<String>,
    detail: impl Into<String>,
    docs: impl Into<String>,
    kind: i32,
    insert: impl Into<String>,
    tier: char,
) -> CompletionItem {
    let label = label.into();
    CompletionItem {
        sort_text: format!("{tier}{label}"),
        label,
        detail: detail.into(),
        documentation: docs.into(),
        kind,
        insert_text: insert.into(),
        data: None,
    }
}

// ---------------------------------------------------------------------------
// Context arms
// ---------------------------------------------------------------------------

/// `alias.` — the columns of whatever the qualifier resolves to.
pub fn qualifier_column_items(
    ctx: &DbtContext,
    doc: &Document,
    scope: &ScopeIndex,
    qualifier: &str,
) -> Vec<CompletionItem> {
    let target = scope.resolve(qualifier).cloned().or_else(|| {
        doc.parsed
            .def_tokens
            .contains_key(qualifier)
            .then(|| RelationTarget::Cte(qualifier.to_string()))
    });
    match target {
        Some(target) => target_column_items(ctx, doc, &target, '0'),
        None => Vec::new(),
    }
}

/// After FROM/JOIN: CTEs plainly, models and source tables as Jinja snippets.
pub fn relation_items(ctx: &DbtContext, doc: &Document) -> Vec<CompletionItem> {
    let mut items: Vec<CompletionItem> = doc
        .parsed
        .def_tokens
        .keys()
        .map(|name| {
            item(
                name.to_string(),
                "CTE in this file",
                "",
                completion_kind::VARIABLE,
                name.to_string(),
                'a',
            )
        })
        .collect();
    for (name, model) in &ctx.model_detail_map {
        let mut relation = item(
            name.clone(),
            format!("Project: {}", model.project_name),
            "",
            completion_kind::REFERENCE,
            format!("{{{{ ref('{name}') }}}}"),
            'c',
        );
        relation.data = Some(serde_json::json!({"kind": "model", "name": name}));
        items.push(relation);
    }
    for source in ctx.source_detail_map.values() {
        for table in source.tables.values() {
            let mut relation = item(
                format!("{} - {}", source.name, table.name),
                format!("Source: {}", source.name),
                "",
                completion_kind::REFERENCE,
                format!("{{{{ source('{}', '{}') }}}}", source.name, table.name),
                'c',
            );
            relation.data = Some(
                serde_json::json!({"kind": "source", "name": source.name, "table": table.name}),
            );
            items.push(relation);
        }
    }
    items
}

/// Select list / predicates: columns in scope first, then locals, functions,
/// demoted keywords.
pub fn column_position_items(
    ctx: &DbtContext,
    doc: &Document,
    pos: Position,
) -> Vec<CompletionItem> {
    let scope = ScopeIndex::build(doc);
    let mut items = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for target in scope.targets() {
        for column in target_column_items(ctx, doc, target, 'a') {
            if seen.insert((column.label.clone(), column.detail.clone())) {
                items.push(column);
            }
        }
    }
    items.extend(
        relation_column_items(ctx, doc)
            .into_iter()
            .filter(|c| seen.insert((c.label.clone(), c.detail.clone()))),
    );
    items.extend(local_identifier_items(doc, pos));
    items.extend(retier(ctx.dialect.function_completion_items(), 'm'));
    items.extend(keyword_items());
    items
}

pub fn keyword_continuation_items(words: &[&'static str]) -> Vec<CompletionItem> {
    words
        .iter()
        .map(|word| {
            item(
                *word,
                "SQL keyword",
                "",
                completion_kind::KEYWORD,
                *word,
                '0',
            )
        })
        .chain(keyword_items())
        .collect()
}

/// Broad write-assist set (the old fallback, unchanged behavior).
pub fn general_items(ctx: &DbtContext, doc: &Document, pos: Position) -> Vec<CompletionItem> {
    let mut items = ctx.dialect.function_completion_items();
    items.extend(keyword_items());
    items.extend(local_identifier_items(doc, pos));
    items.extend(relation_column_items(ctx, doc));
    items
}

// ---------------------------------------------------------------------------
// Shared providers
// ---------------------------------------------------------------------------

fn target_column_items(
    ctx: &DbtContext,
    doc: &Document,
    target: &RelationTarget,
    tier: char,
) -> Vec<CompletionItem> {
    match target {
        RelationTarget::Model(name) => ctx
            .model_detail_map
            .get(name)
            .map(|model| {
                model
                    .columns
                    .iter()
                    .map(|c| {
                        item(
                            c.name.clone(),
                            format!("Column of {name}"),
                            c.description.clone(),
                            completion_kind::FIELD,
                            c.name.clone(),
                            tier,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default(),
        RelationTarget::SourceTable { source, table } => ctx
            .source_detail_map
            .get(source)
            .and_then(|s| s.tables.get(table))
            .map(|t| {
                t.columns
                    .iter()
                    .map(|c| {
                        item(
                            c.name.clone(),
                            format!("Column of {source}.{table}"),
                            c.description.clone(),
                            completion_kind::FIELD,
                            c.name.clone(),
                            tier,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default(),
        RelationTarget::Cte(name) => cte_columns(doc, name)
            .into_iter()
            .map(|column| {
                item(
                    column.clone(),
                    format!("Column of {name} (CTE)"),
                    "",
                    completion_kind::FIELD,
                    column,
                    tier,
                )
            })
            .collect(),
        RelationTarget::Unknown(_) => Vec::new(),
    }
}

/// SQL keywords from sqlparser's global table (covers the Snowflake set —
/// QUALIFY, ILIKE, …), demoted below everything local.
pub fn keyword_items() -> impl Iterator<Item = CompletionItem> {
    sqlparser::keywords::ALL_KEYWORDS.iter().map(|kw| {
        let word = kw.to_lowercase();
        item(
            word.clone(),
            "SQL keyword",
            "",
            completion_kind::KEYWORD,
            word,
            'z',
        )
    })
}

/// CTE names and every identifier already written in the document — minus
/// the word under the cursor.
pub fn local_identifier_items(doc: &Document, pos: Position) -> Vec<CompletionItem> {
    let under_cursor = doc
        .parsed
        .find_token_at_cursor(pos.line, pos.character.saturating_sub(1));
    let mut seen = std::collections::BTreeSet::new();
    let mut items = Vec::new();

    for name in doc.parsed.def_tokens.keys() {
        if seen.insert(name.to_string()) {
            items.push(item(
                name.to_string(),
                "CTE in this file",
                "",
                completion_kind::VARIABLE,
                name.to_string(),
                'a',
            ));
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
            items.push(item(
                literal.clone(),
                "identifier in this file",
                "",
                completion_kind::TEXT,
                literal,
                'b',
            ));
        }
    }
    items
}

/// Documented columns of every model this document refs and every source
/// table it reads, straight from the properties YAML.
pub fn relation_column_items(ctx: &DbtContext, doc: &Document) -> Vec<CompletionItem> {
    let mut seen = std::collections::BTreeSet::new();
    let mut items = Vec::new();
    let mut add = |relation: &str, column: &cogitator_schemas::ColumnEntry| {
        if seen.insert((relation.to_string(), column.name.clone())) {
            items.push(item(
                column.name.clone(),
                format!("Column of {relation}"),
                column.description.clone(),
                completion_kind::FIELD,
                column.name.clone(),
                'a',
            ));
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

fn retier(items: Vec<CompletionItem>, tier: char) -> Vec<CompletionItem> {
    items
        .into_iter()
        .map(|mut item| {
            item.sort_text = format!("{tier}{}", item.label);
            item
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Jinja trigger providers (unchanged behavior, moved from the flat module)
// ---------------------------------------------------------------------------

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
            data: Some(serde_json::json!({"kind": "model", "name": name})),
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
                    serde_json::json!({"kind": "source", "name": source.name, "table": table.name}),
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
            data: Some(serde_json::json!({"kind": "variable", "name": name})),
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
                data: Some(serde_json::json!({
                    "kind": "macro", "name": m.name, "package": m.project_name,
                })),
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
