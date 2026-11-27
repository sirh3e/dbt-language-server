//! Inline column lineage (`inlay.lineage`): after each output column of a
//! model's final select, where it comes from — `← stg_payments.amount`,
//! `← stg_orders.order_id, stg_payments.order_id`, read through CTEs down
//! to the upstream model or source column, `· derived` when the value is
//! computed somewhere on the way. Each upstream reference is a label part
//! whose `location` is that column's definition, so go-to works on it.
//!
//! Pure over a [`ModelLineage`] the shell already holds (never computed
//! here) and a [`Locate`] for the parts' locations. Only lineage read off
//! the template places hints: compiled-route answers have no template
//! spans (`ColumnLineage::end`).

use cogitator_lsp::{inlay_hint_kind, InlayHintLabelPart, PartedInlayHint, Position, Range};
use cogitator_parser::LineTable;

use crate::lineage::{ColumnLineage, ModelLineage};

mod label;
mod locate;
#[cfg(test)]
mod tests;

pub use locate::{Locate, ProjectLocator};

/// The longest label in characters before it is cut with `…`.
pub const DEFAULT_MAX_LEN: usize = 60;

/// cogitator.yml `inlay.lineage` / `inlay.lineage_max_len`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineageInlayOptions {
    pub enabled: bool,
    pub max_len: usize,
}

impl Default for LineageInlayOptions {
    fn default() -> Self {
        LineageInlayOptions {
            enabled: true,
            max_len: DEFAULT_MAX_LEN,
        }
    }
}

/// The lineage hints of `text` within `range` (all-zero: the whole file).
pub fn lineage_inlay_hints(
    text: &str,
    lineage: &ModelLineage,
    range: Range,
    options: LineageInlayOptions,
    locate: &mut dyn Locate,
) -> Vec<PartedInlayHint> {
    if !options.enabled {
        return Vec::new();
    }
    let table = LineTable::new(text);
    let whole = range.is_zero();
    placed_groups(&lineage.columns)
        .into_iter()
        .filter_map(|(end, columns)| {
            let at = hint_position(text, &table, end)?;
            if !whole && !(range.start.line <= at.line && at.line <= range.end.line) {
                return None;
            }
            let label = label::label(&columns, options.max_len, locate)?;
            Some(PartedInlayHint {
                position: at,
                label: label.parts,
                kind: Some(inlay_hint_kind::TYPE),
                padding_left: true,
                padding_right: false,
                tooltip: Some(label.tooltip),
            })
        })
        .collect()
}

/// The columns with a template position, grouped by it (an expanded star
/// is several columns at one select item), in document order.
fn placed_groups(columns: &[ColumnLineage]) -> Vec<(u32, Vec<&ColumnLineage>)> {
    let mut groups: Vec<(u32, Vec<&ColumnLineage>)> = Vec::new();
    for column in columns {
        let Some(end) = column.end else {
            continue;
        };
        match groups.iter_mut().find(|(at, _)| *at == end) {
            Some((_, group)) => group.push(column),
            None => groups.push((end, vec![column])),
        }
    }
    groups.sort_by_key(|(end, _)| *end);
    groups
}

/// After the item, or past a `,` that alone follows it on its line: the
/// hint reads at the end of `o.amount * 100 as amount_cents,`.
fn hint_position(text: &str, table: &LineTable, end: u32) -> Option<Position> {
    let rest = text.get(end as usize..)?;
    let line_rest = rest.split('\n').next().unwrap_or(rest);
    let trimmed = line_rest.trim_start();
    let after_comma = trimmed
        .strip_prefix(',')
        .filter(|tail| tail.trim().is_empty() || tail.trim_start().starts_with("--"));
    let offset = match after_comma {
        Some(_) => end + (line_rest.len() - trimmed.len()) as u32 + 1,
        None => end,
    };
    position(text, table, offset)
}

fn position(text: &str, table: &LineTable, offset: u32) -> Option<Position> {
    if offset as usize > text.len() || !text.is_char_boundary(offset as usize) {
        return None;
    }
    let (line, column) = table.line_col(offset);
    let start = (offset - column) as usize;
    let character = text.get(start..offset as usize)?.encode_utf16().count() as u32;
    Some(Position { line, character })
}

/// A plain part: no tooltip, location or command.
fn plain(value: impl Into<String>) -> InlayHintLabelPart {
    InlayHintLabelPart {
        value: value.into(),
        ..Default::default()
    }
}
