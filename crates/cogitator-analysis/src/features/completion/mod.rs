//! Completion engine (03 §9.4 + FEATURE_REQUESTS "write assist"): the Jinja
//! triggers stay regex-precise; everything else is classified grammatically
//! from the token stream (`context`), resolved against the document's
//! FROM/JOIN scope (`scope`) and served by composable `providers` with
//! tiered ranking.

pub(crate) mod column_info;
mod column_items;
mod context;
mod dbt_calls;
mod details;
mod providers;
pub(crate) mod scope;
mod snippets;

use std::sync::LazyLock;

use cogitator_lsp::{CompletionItem, Position};
use regex::Regex;

use super::{display_value, DbtContext, Document};
use context::CompletionContext;
use scope::sql::{self, Clause, Lookup};
pub use snippets::ModelHints;

// Q1 fixed: `[a-zA-Z0-9_]` instead of Go's `[a-zA-z]` typo.
static REF_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\bref\(('|")[a-zA-Z0-9_]*$"#).unwrap());
static SOURCE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\bsource\(('|")[a-zA-Z0-9_]*$"#).unwrap());
static VAR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\bvar\(('|")[a-zA-Z0-9_]*$"#).unwrap());
static DOC_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\bdoc\(('|")[a-zA-Z0-9_]*$"#).unwrap());
static FUNCTION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\bfunction\(('|")[a-zA-Z0-9_]*$"#).unwrap());

pub fn completion(ctx: &DbtContext, doc: &Document, pos: Position) -> Vec<CompletionItem> {
    completion_with(ctx, doc, pos, &|_| None, &ModelHints::default())
}

/// [`completion`] with a way to read a file (the models a column position
/// reads, when their YAML documents no columns, are asked their column
/// lineage) and what the shell knows of the project from its caches (the
/// inferred columns, the manifest's config, the catalog) for the
/// model-aware snippets and the columns' types.
pub fn completion_with(
    ctx: &DbtContext,
    doc: &Document,
    pos: Position,
    read: &dyn Fn(&std::path::Path) -> Option<String>,
    hints: &ModelHints,
) -> Vec<CompletionItem> {
    let schemas = hints.snapshot.as_deref().map(|s| &*s.schemas);
    complete(ctx, doc, pos, read, hints, schemas)
}

/// The items at `pos`: the engine's, then the model-aware snippets, then
/// every item's details.
fn complete(
    ctx: &DbtContext,
    doc: &Document,
    pos: Position,
    read: &dyn Fn(&std::path::Path) -> Option<String>,
    hints: &ModelHints,
    schemas: Option<&cogitator_ir::schema::ProjectSchemas>,
) -> Vec<CompletionItem> {
    let mut items = engine(ctx, doc, pos, read, schemas);
    if let Some((snippets, exclusive)) = model_snippets(ctx, doc, pos, hints) {
        if exclusive {
            return snippets; // inside `{% if `: nothing else fits
        }
        items.extend(snippets);
    }
    details::decorate(ctx, doc, hints, &mut items);
    items
}

/// The snippets at `pos`, and whether they are all that fits there.
fn model_snippets(
    ctx: &DbtContext,
    doc: &Document,
    pos: Position,
    hints: &ModelHints,
) -> Option<(Vec<CompletionItem>, bool)> {
    let model = super::model_name_from_uri(&doc.uri)?;
    let line = doc.text.split('\n').nth(pos.line as usize)?;
    let cursor = (pos.character as usize).min(line.len());
    let cursor = (0..=cursor).rev().find(|&i| line.is_char_boundary(i))?;
    let offset = doc
        .text
        .split('\n')
        .take(pos.line as usize)
        .map(|l| l.len() + 1)
        .sum::<usize>()
        + cursor;
    let (before, after) = line.split_at(cursor);
    let items = snippets::snippet_items(ctx, doc, &model, before, after, offset, hints);
    // inside `{% if …`: the snippet completes the tag, nothing else fits
    let exclusive = before.rfind("{%") > before.rfind("%}");
    (!items.is_empty()).then_some((items, exclusive))
}

/// [`completion_with`] over the project's published column schemas: the
/// columns of a model or source in scope are resolved through the whole
/// project (upstream `select *`, CTEs, catalog and YAML fill-in) with their
/// types, rather than read off that one relation's YAML or lineage.
pub fn completion_resolving(
    ctx: &DbtContext,
    doc: &Document,
    pos: Position,
    read: &dyn Fn(&std::path::Path) -> Option<String>,
    schemas: Option<&cogitator_ir::schema::ProjectSchemas>,
) -> Vec<CompletionItem> {
    complete(ctx, doc, pos, read, &ModelHints::default(), schemas)
}

fn engine(
    ctx: &DbtContext,
    doc: &Document,
    pos: Position,
    read: &dyn Fn(&std::path::Path) -> Option<String>,
    schemas: Option<&cogitator_ir::schema::ProjectSchemas>,
) -> Vec<CompletionItem> {
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
        return providers::var_items(
            ctx.vars_for(&doc.uri),
            &providers::suffix_for(line, after, "var"),
        );
    }
    if DOC_RE.is_match(before) {
        return providers::doc_items(ctx, &providers::suffix_for(line, after, "doc"));
    }
    if FUNCTION_RE.is_match(before) {
        return providers::function_items(ctx, &providers::suffix_for(line, after, "function"));
    }
    // macro suggestions only while actually inside an unclosed `{{ …`
    // (Go matched any `{{` earlier in the line — engine supersedes that)
    if before.rfind("{{") > before.rfind("}}") {
        return providers::macro_items(ctx);
    }

    // Grammar-aware engine for plain SQL: the clause from the parsed
    // statement where it parses, the token classification otherwise.
    let offset = lines[..pos.line as usize]
        .iter()
        .map(|l| l.len() + 1)
        .sum::<usize>()
        + cursor;
    let parsed = sql::analyze(doc, &ctx.dialect, offset);
    let lookup = Lookup {
        ctx,
        doc,
        read,
        schemas,
    };
    let scopes = || column_items::in_scope(&lookup, parsed.as_ref());
    let classified = context::classify(doc, pos, before);
    if let CompletionContext::MemberAccess { qualifier } = &classified {
        return member_access(&lookup, parsed.as_ref(), &scopes(), qualifier)
            .unwrap_or_else(|| providers::general_items(ctx, doc, pos, Vec::new()));
    }
    match parsed.as_ref().map(|p| p.clause) {
        Some(Clause::Relation) => return providers::relation_items(ctx, doc),
        Some(clause) if clause.takes_columns() => {
            let columns = column_items::column_items(&scopes());
            return providers::column_position_items(ctx, doc, pos, columns);
        }
        _ => {}
    }
    match classified {
        CompletionContext::RelationPosition => providers::relation_items(ctx, doc),
        CompletionContext::ColumnPosition => {
            let columns = column_items::column_items(&scopes());
            providers::column_position_items(ctx, doc, pos, columns)
        }
        CompletionContext::KeywordContinuation(words) => {
            providers::keyword_continuation_items(&words)
        }
        _ => {
            let columns = column_items::column_items(&scopes());
            providers::general_items(ctx, doc, pos, columns)
        }
    }
}

/// `qualifier.`: the columns of the relation in scope it names, else of
/// a CTE of that name; `None` when it names neither.
fn member_access(
    lookup: &Lookup<'_>,
    parsed: Option<&sql::SqlScope<'_>>,
    scopes: &[column_items::InScope],
    qualifier: &str,
) -> Option<Vec<CompletionItem>> {
    let columns = match scopes
        .iter()
        .find(|s| s.qualifier.eq_ignore_ascii_case(qualifier))
    {
        Some(scope) => scope.columns.clone(),
        None => match parsed {
            Some(parsed) if parsed.has_cte(qualifier) => parsed.cte_columns(lookup, qualifier),
            _ => lookup
                .doc
                .parsed
                .def_tokens
                .contains_key(qualifier)
                .then(|| {
                    sql::target_columns(lookup, &scope::RelationTarget::Cte(qualifier.to_string()))
                })?,
        },
    };
    (!columns.is_empty()).then(|| column_items::member_items(&columns))
}

/// completionItem/resolve (P2): fill documentation lazily from the maps.
/// Unknown or malformed `data` (clients echo it opaquely) resolves to None.
pub fn resolve_completion_documentation(
    ctx: &DbtContext,
    data: &serde_json::Value,
) -> Option<String> {
    resolve_completion_documentation_with(ctx, data, &ModelHints::default()).map(|(docs, _)| docs)
}

/// [`resolve_completion_documentation`] with the caches' facts, and
/// whether the text is Markdown (columns, dialect functions).
pub fn resolve_completion_documentation_with(
    ctx: &DbtContext,
    data: &serde_json::Value,
    hints: &ModelHints,
) -> Option<(String, bool)> {
    let parsed = serde_json::from_value::<cogitator_lsp::CompletionData>(data.clone()).ok()?;
    if let Some(docs) = details::resolve_column(ctx, hints, &parsed) {
        return Some((docs, true));
    }
    let markdown = matches!(parsed, cogitator_lsp::CompletionData::Function { .. });
    resolve_plain(ctx, parsed).map(|docs| (docs, markdown))
}

fn resolve_plain(ctx: &DbtContext, data: cogitator_lsp::CompletionData) -> Option<String> {
    match data {
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
        // resolve carries no document: the root's, else a package's
        cogitator_lsp::CompletionData::Variable { name } => ctx
            .variable_detail_map
            .get(&name)
            .or_else(|| ctx.package_variables.iter().find_map(|p| p.vars.get(&name)))
            .map(|v| {
                format!(
                    "{}\n\n{}",
                    display_value(&v.value),
                    super::var_source_words(v)
                )
            }),
        cogitator_lsp::CompletionData::Doc { name } => {
            ctx.doc_detail_map.get(&name).map(|d| d.content.clone())
        }
        cogitator_lsp::CompletionData::DbtFunction { name } => {
            ctx.function_detail_map.get(&name).map(|f| f.markdown())
        }
        cogitator_lsp::CompletionData::Macro { name, package } => {
            let package = package.as_deref().unwrap_or(&ctx.project.name);
            ctx.macro_detail_map
                .get(package)?
                .get(&name)
                .map(|m| m.description.clone())
        }
        cogitator_lsp::CompletionData::Function { name } => {
            ctx.dialect.function_doc(&name).map(str::to_string)
        }
        // a CTE column's documentation was filled with the item
        cogitator_lsp::CompletionData::Column { .. } => None,
    }
}
