//! textDocument/hover (03 §9.1).

use cogitator_lsp::Position;
use cogitator_parser::TokenKind;

use super::{display_value, DbtContext, Document};
use crate::lineage::{self, ModelLineage};

/// Hover (03 §9.1). Default: empty contents. Computes lineage; a server
/// handler on the dispatcher path uses [`hover_with_lineage`].
pub fn hover(ctx: &DbtContext, doc: &Document, pos: Position) -> String {
    let text = hover_inner(ctx, doc, pos, Lineage::Compute);
    super::dispatch::implementation::with_dispatch_line(ctx, doc, pos, text)
}

/// Hover with the lineage the caller already holds. `None` omits the
/// lineage line; this never computes one.
pub fn hover_with_lineage(
    ctx: &DbtContext,
    doc: &Document,
    pos: Position,
    lineage: Option<&ModelLineage>,
) -> String {
    let text = hover_inner(ctx, doc, pos, Lineage::Given(lineage));
    super::dispatch::implementation::with_dispatch_line(ctx, doc, pos, text)
}

enum Lineage<'a> {
    Compute,
    Given(Option<&'a ModelLineage>),
}

fn hover_inner(ctx: &DbtContext, doc: &Document, pos: Position, source: Lineage<'_>) -> String {
    let Some(idx) = doc.parsed.find_token_at_cursor(pos.line, pos.character) else {
        return String::new();
    };
    let parsed = &doc.parsed;
    if let Some(m) = super::definition::macro_in_string(ctx, parsed, idx, pos) {
        return with_definition(
            m.description.clone(),
            "macro",
            &m.name,
            &m.uri,
            m.range.start.line,
        );
    }
    let idx = super::definition::named_token(parsed, idx);
    let token = &parsed.tokens[idx];
    let literal = parsed.literal(token);

    match token.kind {
        TokenKind::Ref => ctx
            .model_detail_map
            .get(literal)
            .map(|m| with_definition(m.description.clone(), "model", literal, &m.uri, 0))
            .unwrap_or_default(),
        TokenKind::Doc => ctx
            .doc_detail_map
            .get(literal)
            .map(|d| d.content.clone())
            .unwrap_or_default(),
        TokenKind::Function => ctx
            .function_detail_map
            .get(literal)
            .map(|f| f.markdown())
            .unwrap_or_default(),
        TokenKind::SourceName => ctx
            .source_detail_map
            .get(literal)
            .map(|s| {
                with_definition(
                    s.description.clone(),
                    "source",
                    literal,
                    &s.uri,
                    s.range.start.line,
                )
            })
            .unwrap_or_default(),
        TokenKind::SourceTable => {
            let Some(source_token) = parsed.lookback_find(idx, TokenKind::SourceName, 4) else {
                return String::new();
            };
            let source_name = parsed.literal(source_token);
            let Some(source) = ctx.source_detail_map.get(source_name) else {
                return String::new();
            };
            let table = source.tables.get(literal);
            let text = format!(
                "Source: {}\n{}\n\nTable: {}\n{}",
                source.name,
                source.description,
                table.map(|t| t.name.as_str()).unwrap_or_default(),
                table.map(|t| t.description.as_str()).unwrap_or_default(),
            );
            match table.filter(|t| !t.uri.as_os_str().is_empty()) {
                Some(t) => with_link(text, &t.uri, t.range.start.line),
                None => text,
            }
        }
        TokenKind::Var => match ctx.var_for(&doc.uri, literal) {
            Some(v) => format!(
                "{literal}: {}\n\n{}",
                display_value(&v.value),
                super::var_source_words(v)
            ),
            None => match parsed.var_default(idx) {
                Some(default) => format!("{literal}: {default}\n\nundefined; the default applies"),
                None => format!("{literal}: undefined"),
            },
        },
        TokenKind::EnvVar => super::env_vars::env_hover(ctx, doc, idx),
        TokenKind::Macro => {
            let package = parsed
                .lookback_find(idx, TokenKind::Package, 2)
                .map(|t| parsed.literal(t).to_string())
                .unwrap_or_else(|| ctx.project.name.clone());
            ctx.macro_detail_map
                .get(&package)
                .and_then(|m| m.get(literal))
                .map(|m| {
                    with_definition(
                        m.description.clone(),
                        "macro",
                        literal,
                        &m.uri,
                        m.range.start.line,
                    )
                })
                .unwrap_or_default()
        }
        _ if doc.parsed.def_tokens.contains_key(literal) => {
            // a CTE of this model, at its definition or a reference:
            // its columns and where each comes from
            super::cte_card(doc, literal, &ctx.dialect)
                .map(|card| super::card_markdown(&card))
                .unwrap_or_default()
        }
        _ => {
            // documented column of a relation in this document? (editor-
            // experience feature: column hover, with lineage when derivable)
            if let Some(hit) = super::completion::scope::resolve_column(ctx, doc, literal) {
                // the same text the column's completion item documents
                let fact = super::completion::column_info::column_fact(
                    ctx,
                    &super::completion::ModelHints::default(),
                    &hit.of,
                    literal,
                    &[],
                );
                let mut text = super::completion::column_info::markdown(&fact);
                let computed;
                let lineage = match source {
                    Lineage::Compute => {
                        computed = lineage::column_lineage(&doc.parsed, &ctx.dialect);
                        computed.as_ref()
                    }
                    Lineage::Given(given) => given,
                };
                if let Some(lineage) = lineage {
                    let sources: Vec<String> = lineage
                        .columns
                        .iter()
                        .filter(|c| c.name == literal || c.name == "*")
                        .flat_map(|c| c.sources.iter())
                        .map(|s| match s {
                            crate::lineage::SourceRef::Model { name, column } => {
                                format!("{name}.{column}")
                            }
                            crate::lineage::SourceRef::Source {
                                source,
                                table,
                                column,
                            } => {
                                format!("{source}.{table}.{column}")
                            }
                            crate::lineage::SourceRef::Unknown { name, column } => {
                                format!("{name}.{column}")
                            }
                        })
                        .filter(|s| s.ends_with(&format!(".{literal}")) || !s.ends_with(".*"))
                        .collect();
                    if !sources.is_empty() {
                        text.push_str("\n\nLineage: ");
                        text.push_str(&sources.join(", "));
                        text.push_str(lineage.origin_note());
                    }
                }
                return text;
            }
            ctx.dialect
                .function_doc(literal)
                .map(str::to_string)
                .unwrap_or_default()
        }
    }
}

/// A hover's text under a line naming what it is, and above the line
/// that says where it is defined.
fn with_definition(
    text: String,
    kind: &str,
    name: &str,
    path: &std::path::Path,
    line: u32,
) -> String {
    let head = format!("**{kind}** `{name}`");
    let body = if text.trim().is_empty() {
        head
    } else {
        format!("{head}\n\n{text}")
    };
    with_link(body, path, line)
}

/// `text` and a last line linking the definition, which the editor's
/// hover shows as a link (Neovim's plugin jumps there on <CR>).
fn with_link(text: String, path: &std::path::Path, line: u32) -> String {
    if path.as_os_str().is_empty() {
        return text;
    }
    let shown = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    format!(
        "{text}\n\nDefinition: [{shown}:{}]({}#L{}) · `gd` jumps there",
        line + 1,
        super::file_uri(path),
        line + 1
    )
}
