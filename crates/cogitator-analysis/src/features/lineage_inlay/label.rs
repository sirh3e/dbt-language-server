//! The label of one hint: `← ` then each upstream column as its own part,
//! `, `-separated, cut at the length cap with `…`, and ` · derived` kept
//! whatever the cut. The tooltip always holds the whole answer.

use cogitator_lsp::InlayHintLabelPart;

use super::{plain, Locate};
use crate::lineage::{ColumnLineage, SourceRef};

const ARROW: &str = "← ";
const SEPARATOR: &str = ", ";
const DERIVED: &str = " · derived";
const ELLIPSIS: &str = "…";

pub(super) struct Label {
    pub parts: Vec<InlayHintLabelPart>,
    pub tooltip: String,
}

/// `None` when the columns name nothing upstream and nothing is derived:
/// there is nothing to say.
pub(super) fn label(
    columns: &[&ColumnLineage],
    max_len: usize,
    locate: &mut dyn Locate,
) -> Option<Label> {
    let derived = columns.iter().any(|c| c.derived);
    let refs = references(columns);
    if refs.is_empty() && !derived {
        return None;
    }
    let tooltip = tooltip(columns, derived);
    if refs.is_empty() {
        return Some(Label {
            parts: vec![plain("← derived")],
            tooltip,
        });
    }
    let suffix = if derived { DERIVED } else { "" };
    let budget = max_len.saturating_sub(chars(ARROW) + chars(suffix));
    let texts: Vec<String> = refs.iter().map(shown).collect();
    let full = texts.iter().map(|t| chars(t)).sum::<usize>()
        + chars(SEPARATOR) * texts.len().saturating_sub(1);
    let mut parts = vec![plain(ARROW)];
    if full <= budget {
        for (i, (text, reference)) in texts.into_iter().zip(&refs).enumerate() {
            if i > 0 {
                parts.push(plain(SEPARATOR));
            }
            parts.push(part(text, reference, locate));
        }
    } else {
        // whole references while they and a trailing `, …` fit
        let mut used = 0;
        let mut kept = 0;
        for (text, reference) in texts.iter().zip(&refs) {
            let sep = if kept == 0 { 0 } else { chars(SEPARATOR) };
            let tail = chars(SEPARATOR) + chars(ELLIPSIS);
            if used + sep + chars(text) + tail > budget {
                break;
            }
            if kept > 0 {
                parts.push(plain(SEPARATOR));
            }
            used += sep + chars(text);
            kept += 1;
            parts.push(part(text.clone(), reference, locate)); // boundary: the part owns its text
        }
        if kept == 0 {
            // not even one fits: the first, cut
            let keep = budget.saturating_sub(chars(ELLIPSIS)).max(1);
            parts.push(part(cut(&texts[0], keep), &refs[0], locate));
        } else {
            parts.push(plain(SEPARATOR));
        }
        parts.push(InlayHintLabelPart {
            value: ELLIPSIS.to_string(),
            tooltip: Some(tooltip.clone()), // boundary: the cut part shows the rest
            ..Default::default()
        });
    }
    if derived {
        parts.push(plain(DERIVED));
    }
    Some(Label { parts, tooltip })
}

/// The distinct upstream columns, in order; several columns at one select
/// item (an expanded star) show their relations, `stg_orders.*`.
fn references(columns: &[&ColumnLineage]) -> Vec<SourceRef> {
    let mut out: Vec<SourceRef> = Vec::new();
    for column in columns {
        for source in &column.sources {
            let source = if columns.len() > 1 {
                star_of(source)
            } else {
                source.clone() // boundary: the label owns its references
            };
            if !out.contains(&source) {
                out.push(source);
            }
        }
    }
    out
}

fn star_of(source: &SourceRef) -> SourceRef {
    let star = "*".to_string();
    match source {
        SourceRef::Model { name, .. } => SourceRef::Model {
            name: name.clone(),
            column: star,
        },
        SourceRef::Source { source, table, .. } => SourceRef::Source {
            source: source.clone(),
            table: table.clone(),
            column: star,
        },
        SourceRef::Unknown { name, .. } => SourceRef::Unknown {
            name: name.clone(),
            column: star,
        },
    }
}

/// `stg_payments.amount`, `jaffle.raw_orders.id`, `raw_table.id?`.
pub(super) fn shown(reference: &SourceRef) -> String {
    match reference {
        SourceRef::Unknown { .. } => format!("{}?", reference.label()),
        _ => reference.label(),
    }
}

fn part(value: String, reference: &SourceRef, locate: &mut dyn Locate) -> InlayHintLabelPart {
    let tooltip = match reference {
        SourceRef::Model { name, .. } => format!("model {name}"),
        SourceRef::Source { source, table, .. } => format!("source {source}.{table}"),
        SourceRef::Unknown { name, .. } => {
            format!("{name} is not a model or source of this project")
        }
    };
    InlayHintLabelPart {
        value,
        tooltip: Some(tooltip),
        location: locate.locate(reference),
        command: None,
    }
}

fn tooltip(columns: &[&ColumnLineage], derived: bool) -> String {
    let mut lines: Vec<String> = columns
        .iter()
        .map(|column| {
            let from: Vec<String> = column.sources.iter().map(shown).collect();
            let from = if from.is_empty() {
                "no upstream column".to_string()
            } else {
                from.join(", ")
            };
            let mark = if column.derived { " (derived)" } else { "" };
            format!("{} ← {from}{mark}", column.name)
        })
        .collect();
    if derived {
        lines.push("derived: computed on the way, not one column passed through".to_string());
    }
    lines.join("\n")
}

fn chars(text: &str) -> usize {
    text.chars().count()
}

fn cut(text: &str, keep: usize) -> String {
    text.chars().take(keep).collect()
}
