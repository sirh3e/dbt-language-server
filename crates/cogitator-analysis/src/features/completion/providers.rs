//! Completion item providers, composed per context with sort-text tiers:
//! `0` context-exact (columns in scope, [`super::column_items`]; after
//! FROM/JOIN: `0` CTEs, `1` refs, `2` sources) · `a` CTEs/locals/columns
//! of the document's other relations · `b` buffer idents · `m` functions
//! · `z` keywords.

use cogitator_lsp::{completion_kind, CompletionData, CompletionItem, Position};
use cogitator_parser::{Keyword, TokenKind};

use super::{DbtContext, Document};

pub(super) use super::dbt_calls::{
    doc_items, macro_items, quote_type, ref_items, source_items, suffix_for, var_items,
};

pub(super) fn item(
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
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Context arms
// ---------------------------------------------------------------------------

/// After FROM/JOIN: CTEs plainly (`0`), then models (`1`) and source
/// tables (`2`) as Jinja snippets.
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
                '0',
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
            '1',
        );
        relation.data = Some(CompletionData::Model { name: name.clone() }.into_value());
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
                '2',
            );
            relation.data = Some(
                CompletionData::Source {
                    name: source.name.clone(),
                    table: table.name.clone(),
                }
                .into_value(),
            );
            items.push(relation);
        }
    }
    items
}

/// Select list / predicates: the columns in scope (qualified and bare,
/// [`super::column_items`]) first, then the document's other relations'
/// columns, locals, functions, demoted keywords.
pub fn column_position_items(
    ctx: &DbtContext,
    doc: &Document,
    pos: Position,
    columns: Vec<CompletionItem>,
) -> Vec<CompletionItem> {
    let mut items = columns;
    let mut seen: std::collections::BTreeSet<String> =
        items.iter().map(|c| c.label.clone()).collect();
    items.extend(
        relation_column_items(ctx, doc)
            .into_iter()
            .filter(|c| seen.insert(c.label.clone())),
    );
    items.extend(
        local_identifier_items(doc, pos)
            .into_iter()
            .filter(|c| seen.insert(c.label.clone())),
    );
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

/// Broad write-assist set: the columns in scope first, then functions,
/// keywords, locals and the document's relations' columns.
pub fn general_items(
    ctx: &DbtContext,
    doc: &Document,
    pos: Position,
    columns: Vec<CompletionItem>,
) -> Vec<CompletionItem> {
    let mut items = columns;
    items.extend(ctx.dialect.function_completion_items());
    items.extend(keyword_items());
    items.extend(local_identifier_items(doc, pos));
    items.extend(relation_column_items(ctx, doc));
    items
}

// ---------------------------------------------------------------------------
// Shared providers
// ---------------------------------------------------------------------------

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
    let mut add =
        |relation: &str, source: Option<&str>, column: &cogitator_schemas::ColumnEntry| {
            let shown = match source {
                Some(source) => format!("{source}.{relation}"),
                None => relation.to_string(),
            };
            if seen.insert((shown.clone(), column.name.clone())) {
                let mut field = item(
                    column.name.clone(),
                    match &column.data_type {
                        Some(t) if !t.is_empty() => format!("{relation} · {t}"),
                        _ => relation.to_string(),
                    },
                    column.description.clone(),
                    completion_kind::FIELD,
                    column.name.clone(),
                    'a',
                );
                // its type from every place one is known, its documentation
                // resolved as Markdown (`details`)
                field.data = Some(
                    CompletionData::Column {
                        relation: relation.to_string(),
                        source: source.map(str::to_string),
                        cte: false,
                        column: column.name.clone(),
                    }
                    .into_value(),
                );
                items.push(field);
            }
        };

    for token in &doc.parsed.tokens {
        match token.kind {
            TokenKind::Ref => {
                let name = doc.parsed.literal(token);
                if let Some(model) = ctx.model_detail_map.get(name) {
                    for column in &model.columns {
                        add(name, None, column);
                    }
                }
            }
            TokenKind::SourceTable => {
                let table_name = doc.parsed.literal(token);
                for source in ctx.source_detail_map.values() {
                    if let Some(table) = source.tables.get(table_name) {
                        for column in &table.columns {
                            add(table_name, Some(&source.name), column);
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
