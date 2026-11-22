//! The plan's catalog-proven CTE row counts as inlay hints, after each
//! CTE name: `1,200 rows` (exact) or `≤ 1,200 rows` (a bound). Pure; the
//! shell decides whether statistics and the toggle allow hints at all.

use cogitator_lsp::{inlay_hint_kind, InlayHint, Range};
use cogitator_parser::LineTable;

use super::diagnostics::span_range;
use super::report::QueryPlan;
use super::row_hints::Rows;

/// One hint per CTE the plan has a proven count for, within `range`
/// (an all-zero range is the whole document).
pub fn row_inlay_hints(text: &str, plan: &QueryPlan, range: Range) -> Vec<InlayHint> {
    if plan.row_hints.is_empty() {
        return Vec::new();
    }
    let table = LineTable::new(text);
    let whole = range.is_zero();
    plan.row_hints
        .iter()
        .filter_map(|hint| {
            let at = span_range(text, &table, hint.span)?.end;
            (whole || (range.start.line <= at.line && at.line <= range.end.line)).then(|| {
                InlayHint {
                    position: at,
                    label: label(hint.rows),
                    kind: Some(inlay_hint_kind::TYPE),
                    padding_left: true,
                    padding_right: false,
                    tooltip: None,
                }
            })
        })
        .collect()
}

pub(crate) fn label(rows: Rows) -> String {
    let noun = if rows.count() == 1 { "row" } else { "rows" };
    match rows {
        Rows::Exact(n) => format!("{} {noun}", grouped(n)),
        Rows::AtMost(n) => format!("≤ {} {noun}", grouped(n)),
    }
}

/// `1234567` → `1,234,567`.
fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let groups: Vec<&str> = digits
        .as_bytes()
        .rchunks(3)
        .rev()
        .filter_map(|chunk| std::str::from_utf8(chunk).ok())
        .collect();
    groups.join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_group_digits_and_say_bounds() {
        for (rows, want) in [
            (Rows::Exact(0), "0 rows"),
            (Rows::Exact(1), "1 row"),
            (Rows::Exact(999), "999 rows"),
            (Rows::Exact(1_000), "1,000 rows"),
            (Rows::AtMost(1_234_567), "≤ 1,234,567 rows"),
            (Rows::AtMost(1), "≤ 1 row"),
        ] {
            assert_eq!(label(rows), want, "{rows:?}");
        }
    }
}
