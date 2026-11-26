//! IntelliSense-style item details, a pass over the finished list: each
//! column gets its type beside its label and its relation further right
//! (`labelDetails`), and models, sources, macros and functions say what
//! they are there (materialization, `source`, a signature). Columns of a
//! model or source keep their documentation lazy (`CompletionData::Column`,
//! resolved by [`resolve_column`]); a CTE's are filled here, as the
//! resolver has no document.

use cogitator_lsp::{CompletionData, CompletionItem, LabelDetails};
use cogitator_parser::TokenKind;

use super::column_info::{column_fact, markdown, ColumnOf};
use super::snippets::ModelHints;
use super::{DbtContext, Document};
use crate::context::ModelKind;

/// The relations the document reads, for CTE columns' inheritance.
fn reads(doc: &Document) -> Vec<ColumnOf> {
    let parsed = &doc.parsed;
    let mut out = Vec::new();
    for (idx, token) in parsed.tokens.iter().enumerate() {
        match token.kind {
            TokenKind::Ref => out.push(ColumnOf::Model(parsed.literal(token).to_string())),
            TokenKind::SourceTable => {
                if let Some(source) = parsed.lookback_find(idx, TokenKind::SourceName, 4) {
                    out.push(ColumnOf::Source {
                        source: parsed.literal(source).to_string(),
                        table: parsed.literal(token).to_string(),
                    });
                }
            }
            _ => {}
        }
    }
    out.dedup();
    out
}

fn of_data(data: &CompletionData) -> Option<(ColumnOf, &str)> {
    let CompletionData::Column {
        relation,
        source,
        cte,
        column,
    } = data
    else {
        return None;
    };
    let of = match (source, cte) {
        (_, true) => ColumnOf::Cte(relation.clone()),
        (Some(source), false) => ColumnOf::Source {
            source: source.clone(),
            table: relation.clone(),
        },
        (None, false) => ColumnOf::Model(relation.clone()),
    };
    Some((of, column))
}

/// Fills the details of every item in place.
pub(crate) fn decorate(
    ctx: &DbtContext,
    doc: &Document,
    hints: &ModelHints,
    items: &mut [CompletionItem],
) {
    let read = reads(doc);
    for item in items.iter_mut() {
        let Some(data) = item
            .data
            .as_ref()
            .and_then(|d| serde_json::from_value::<CompletionData>(d.clone()).ok())
        else {
            continue;
        };
        if let Some((of, column)) = of_data(&data) {
            let fact = column_fact(ctx, hints, &of, column, &read);
            let relation = of.label();
            if let Some((ty, _)) = &fact.ty {
                // a type only the catalog or the schema job knows: in
                // `detail` too, in the engine's `relation · type`
                if item.detail == relation {
                    item.detail = format!("{relation} · {ty}");
                }
            }
            item.label_details = Some(LabelDetails {
                detail: fact.ty.as_ref().map(|(t, _)| format!(" {t}")),
                description: Some(relation),
            });
            if let ColumnOf::Cte(_) = of {
                item.documentation = markdown(&fact);
                item.markdown = true;
                item.data = None;
            } else if item.documentation.is_empty() {
                // plain until resolved: a client that never resolves
                // still shows the description (an inherited one too)
                item.documentation = fact.description.clone();
            }
            continue;
        }
        item.label_details = other_details(ctx, hints, &data);
    }
}

fn other_details(
    ctx: &DbtContext,
    hints: &ModelHints,
    data: &CompletionData,
) -> Option<LabelDetails> {
    let description = |text: String| {
        Some(LabelDetails {
            detail: None,
            description: Some(text),
        })
    };
    match data {
        CompletionData::Model { name } => {
            let details = ctx.model_detail_map.get(name)?;
            let configured = hints
                .manifest
                .as_deref()
                .and_then(|m| m.models.get(name))
                .and_then(|n| n.config.materialized.clone());
            description(configured.unwrap_or_else(|| {
                match details.kind {
                    ModelKind::Model => "model",
                    ModelKind::Seed => "seed",
                    ModelKind::Snapshot => "snapshot",
                }
                .to_string()
            }))
        }
        CompletionData::Source { .. } => description("source".to_string()),
        CompletionData::Macro { name, package } => {
            let package = package.as_deref().unwrap_or(&ctx.project.name);
            let m = ctx.macro_detail_map.get(package)?.get(name)?;
            let params: Vec<String> = m
                .params
                .iter()
                .map(|p| match &p.default {
                    Some(d) => format!("{}={d}", p.name),
                    None => p.name.clone(),
                })
                .collect();
            Some(LabelDetails {
                detail: Some(format!("({})", params.join(", "))),
                description: Some(package.to_string()),
            })
        }
        CompletionData::Function { name } => {
            let doc = ctx.dialect.function_doc(name)?;
            Some(LabelDetails {
                detail: function_signature(doc),
                description: None,
            })
        }
        _ => None,
    }
}

/// `( <num_expr> )` out of a function doc's first code line, `ABS( <num_expr> )`.
pub(crate) fn function_signature(doc: &str) -> Option<String> {
    let line = doc
        .lines()
        .find(|l| !l.trim().is_empty() && !l.trim_start().starts_with("```"))?;
    let open = line.find('(')?;
    let close = line.rfind(')')?;
    (close > open).then(|| line[open..=close].trim().to_string())
}

/// `completionItem/resolve` of a model's or source's column: its fact as
/// Markdown. `None` for any other payload (and for a CTE's, filled eagerly).
pub(crate) fn resolve_column(
    ctx: &DbtContext,
    hints: &ModelHints,
    data: &CompletionData,
) -> Option<String> {
    let (of, column) = of_data(data)?;
    if let ColumnOf::Cte(_) = of {
        return None;
    }
    Some(markdown(&column_fact(ctx, hints, &of, column, &[])))
}

#[cfg(test)]
mod tests {
    use super::function_signature;

    #[test]
    fn the_signature_is_the_first_code_line_s_parentheses() {
        assert_eq!(
            function_signature("```sql\nABS( <num_expr> )\n```\n[docs](x)").as_deref(),
            Some("( <num_expr> )")
        );
        assert_eq!(function_signature("no call here"), None);
    }
}
