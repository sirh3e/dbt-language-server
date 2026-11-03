//! Completion engine (03 §9.4 + FEATURE_REQUESTS "write assist"): the Jinja
//! triggers stay regex-precise; everything else is classified grammatically
//! from the token stream (`context`), resolved against the document's
//! FROM/JOIN scope (`scope`) and served by composable `providers` with
//! tiered ranking.

mod context;
mod providers;
pub(crate) mod scope;

use std::sync::LazyLock;

use cogitator_lsp::{CompletionItem, Position};
use regex::Regex;

use super::{display_value, DbtContext, Document};
use context::CompletionContext;
use scope::ScopeIndex;

// Q1 fixed: `[a-zA-Z0-9_]` instead of Go's `[a-zA-z]` typo.
static REF_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\bref\(('|")[a-zA-Z0-9_]*$"#).unwrap());
static SOURCE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\bsource\(('|")[a-zA-Z0-9_]*$"#).unwrap());
static VAR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\bvar\(('|")[a-zA-Z0-9_]*$"#).unwrap());

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

    // Jinja call triggers: precise, highest priority (unchanged behavior).
    if REF_RE.is_match(before) {
        return providers::ref_items(ctx, &providers::suffix_for(line, after, "ref"));
    }
    if SOURCE_RE.is_match(before) {
        return providers::source_items(
            ctx,
            &providers::suffix_for(line, after, "source"),
            &providers::quote_type(before),
        );
    }
    if VAR_RE.is_match(before) {
        return providers::var_items(ctx, &providers::suffix_for(line, after, "var"));
    }
    // macro suggestions only while actually inside an unclosed `{{ …`
    // (Go matched any `{{` earlier in the line — engine supersedes that)
    if before.rfind("{{") > before.rfind("}}") {
        return providers::macro_items(ctx);
    }

    // Grammar-aware engine for plain SQL.
    match context::classify(doc, pos, before) {
        CompletionContext::MemberAccess { qualifier } => {
            let scope = ScopeIndex::build(doc);
            let items = providers::qualifier_column_items(ctx, doc, &scope, &qualifier);
            if !items.is_empty() {
                return items;
            }
            providers::general_items(ctx, doc, pos) // unresolved alias → broad set
        }
        CompletionContext::RelationPosition => providers::relation_items(ctx, doc),
        CompletionContext::ColumnPosition => providers::column_position_items(ctx, doc, pos),
        CompletionContext::KeywordContinuation(words) => {
            providers::keyword_continuation_items(&words)
        }
        CompletionContext::General => providers::general_items(ctx, doc, pos),
    }
}

/// completionItem/resolve (P2): fill documentation lazily from the maps.
/// Unknown or malformed `data` (clients echo it opaquely) resolves to None.
pub fn resolve_completion_documentation(
    ctx: &DbtContext,
    data: &serde_json::Value,
) -> Option<String> {
    match serde_json::from_value::<cogitator_lsp::CompletionData>(data.clone()).ok()? {
        cogitator_lsp::CompletionData::Model { name } => ctx
            .model_detail_map
            .get(&name)
            .map(|m| m.description.clone()),
        cogitator_lsp::CompletionData::Source { name, table } => {
            let source = ctx.source_detail_map.get(&name)?;
            let t = source.tables.get(&table)?;
            Some(format!(
                "{}\n\nTable: {}\n{}",
                source.description, t.name, t.description
            ))
        }
        cogitator_lsp::CompletionData::Variable { name } => ctx
            .variable_detail_map
            .get(&name)
            .map(|v| display_value(&v.value)),
        cogitator_lsp::CompletionData::Macro { name, package } => {
            let package = package.as_deref().unwrap_or(&ctx.project.name);
            ctx.macro_detail_map
                .get(package)?
                .get(&name)
                .map(|m| m.description.clone())
        }
        cogitator_lsp::CompletionData::Function { name } => {
            ctx.dialect.function_docs().get(&name).cloned()
        }
    }
}
