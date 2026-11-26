//! What a CTE is, without running anything: its columns and where each
//! comes from, for the hover on a CTE's name (its definition or any
//! reference to it) and for the CTE picker. The lineage is the model's
//! own lineage engine run on the model's template cut after the CTE —
//! the later CTEs and the final select dropped, `select * from <cte>`
//! appended — so a CTE's columns trace through the CTEs before it to
//! models and sources, exactly as the model's do. Pure.

use cogitator_dialects::Dialect;
use cogitator_parser::{DocumentParser, JinjaSqlParser, TokenKind};
use serde::Serialize;

use crate::lineage::{self, SourceRef};
use crate::Document;

/// One CTE as the card shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CteCard {
    pub name: String,
    pub columns: Vec<CteColumn>,
    /// Whether the columns come from the lineage engine (exact) or were
    /// read off the CTE's select list (when the lineage declined).
    pub traced: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CteColumn {
    pub name: String,
    /// `model.column`, `source.table.column` or `relation.column`.
    pub from: Vec<String>,
}

/// The template up to the end of `name`'s body, then `select * from
/// <name>`; `None` when `name` is no CTE of the document.
pub(crate) fn cut_at(doc: &Document, name: &str) -> Option<String> {
    let def = doc.parsed.def_tokens.get(name)?;
    let tokens = &doc.parsed.tokens;
    let start = tokens.iter().position(|t| t.start == def.start)?;
    let mut depth = 0i32;
    let mut seen_open = false;
    let mut close = None;
    for token in &tokens[start..] {
        match token.kind {
            TokenKind::Symbol('(') => {
                depth += 1;
                seen_open = true;
            }
            TokenKind::Symbol(')') => {
                depth -= 1;
                if seen_open && depth == 0 {
                    close = Some(token.end as usize);
                    break;
                }
            }
            _ => {}
        }
    }
    let close = close?;
    Some(format!("{}\nselect * from {name}\n", &doc.text[..close]))
}

/// The card of the CTE `name`.
pub fn cte_card(doc: &Document, name: &str, dialect: &Dialect) -> Option<CteCard> {
    doc.parsed.def_tokens.get(name)?;
    let traced = cut_at(doc, name)
        .and_then(|text| JinjaSqlParser.parse(&text, dialect).ok())
        .and_then(|parsed| lineage::column_lineage_expanded(&parsed, dialect))
        .filter(|l| !l.columns.is_empty() && l.columns.iter().all(|c| c.name != "*"));
    Some(match traced {
        Some(lineage) => CteCard {
            name: name.to_string(),
            columns: lineage
                .columns
                .iter()
                .map(|c| CteColumn {
                    name: c.name.clone(),
                    from: c.sources.iter().map(source_text).collect(),
                })
                .collect(),
            traced: true,
        },
        None => CteCard {
            name: name.to_string(),
            columns: crate::features::completion::scope::cte_columns(doc, name)
                .into_iter()
                .map(|name| CteColumn {
                    name,
                    from: Vec::new(),
                })
                .collect(),
            traced: false,
        },
    })
}

fn source_text(source: &SourceRef) -> String {
    match source {
        SourceRef::Model { name, column } | SourceRef::Unknown { name, column } => {
            format!("{name}.{column}")
        }
        SourceRef::Source {
            source,
            table,
            column,
        } => format!("{source}.{table}.{column}"),
    }
}

/// The hover's markdown.
pub fn card_markdown(card: &CteCard) -> String {
    let count = card.columns.len();
    let mut out = format!(
        "**CTE** `{}` · {count} column{}\n",
        card.name,
        if count == 1 { "" } else { "s" }
    );
    if card.columns.is_empty() {
        out.push_str("\nIts columns could not be read.\n");
        return out;
    }
    if card.traced {
        out.push_str("\n| column | from |\n|---|---|\n");
        for c in &card.columns {
            let from = if c.from.is_empty() {
                "computed".to_string()
            } else {
                c.from
                    .iter()
                    .map(|f| format!("`{f}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            out.push_str(&format!("| `{}` | {from} |\n", c.name));
        }
    } else {
        out.push('\n');
        for c in &card.columns {
            out.push_str(&format!("- `{}`\n", c.name));
        }
        out.push_str(
            "\n_Read from its select list: the lineage declines on this model \
             (Jinja control flow, or SQL it cannot bind)._\n",
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Documents;
    use pretty_assertions::assert_eq;

    fn doc(text: &str) -> Document {
        let mut docs = Documents::default();
        let uri = "file:///p/models/m.sql".to_string();
        docs.open(uri.clone(), text, &Dialect::default());
        let d = docs.get(&uri).unwrap();
        Document {
            uri: d.uri.clone(),
            text: d.text.clone(),
            parsed: d.parsed.clone(),
            generation: d.generation,
        }
    }

    const MODEL: &str = "{{ config(materialized='table') }}\n\
        with orders as (\n    select order_id, customer_id, amount from {{ ref('stg_orders') }}\n),\n\
        totals as (\n    select customer_id, sum(amount) as total from orders group by customer_id\n)\n\
        select * from totals\n";

    #[test]
    fn hover_on_a_cte_reference_is_its_card() {
        let text =
            "with orders as (\n    select order_id, customer_id from {{ ref('stg_orders') }}\n),\n\
                    big as (\n    select order_id from orders\n)\n\
                    select * from big\n";
        let d = doc(text);
        let ctx = crate::DbtContext::default();
        let pos = cogitator_lsp::Position {
            line: 4,
            character: 27,
        };
        let shown = crate::features::hover(&ctx, &d, pos);
        assert!(
            shown.starts_with("**CTE** `orders` · 2 columns"),
            "{shown:?}"
        );
        // at its definition too
        let at_def = crate::features::hover(
            &ctx,
            &d,
            cogitator_lsp::Position {
                line: 0,
                character: 6,
            },
        );
        assert!(at_def.starts_with("**CTE** `orders`"), "{at_def:?}");
    }

    #[test]
    fn the_cut_keeps_the_ctes_up_to_the_one_asked() {
        let d = doc(MODEL);
        let cut = cut_at(&d, "orders").unwrap();
        assert!(
            cut.ends_with("{{ ref('stg_orders') }}\n)\nselect * from orders\n"),
            "{cut}"
        );
        assert!(!cut.contains("totals"), "{cut}");
        assert_eq!(cut_at(&d, "nope"), None);
    }

    #[test]
    fn a_cte_traces_to_its_models() {
        let card = cte_card(&doc(MODEL), "orders", &Dialect::default()).unwrap();
        assert!(card.traced);
        assert_eq!(
            card.columns,
            vec![
                CteColumn {
                    name: "order_id".into(),
                    from: vec!["stg_orders.order_id".into()]
                },
                CteColumn {
                    name: "customer_id".into(),
                    from: vec!["stg_orders.customer_id".into()]
                },
                CteColumn {
                    name: "amount".into(),
                    from: vec!["stg_orders.amount".into()]
                },
            ]
        );
    }

    #[test]
    fn a_later_cte_traces_through_the_earlier_ones() {
        let card = cte_card(&doc(MODEL), "totals", &Dialect::default()).unwrap();
        let names: Vec<_> = card.columns.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["customer_id", "total"]);
        assert_eq!(card.columns[1].from, vec!["stg_orders.amount".to_string()]);
        let md = card_markdown(&card);
        assert!(md.starts_with("**CTE** `totals` · 2 columns\n"), "{md}");
        assert!(md.contains("| `total` | `stg_orders.amount` |"), "{md}");
    }

    #[test]
    fn control_flow_falls_back_to_the_select_list() {
        let text = "with a as (\n  select x, y from {{ ref('m') }}\n  {% if is_incremental() %}where x > 1{% endif %}\n)\nselect * from a\n";
        let card = cte_card(&doc(text), "a", &Dialect::default()).unwrap();
        assert!(!card.traced);
        let names: Vec<_> = card.columns.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["x", "y"]);
        assert!(card_markdown(&card).contains("_Read from its select list"));
    }
}
