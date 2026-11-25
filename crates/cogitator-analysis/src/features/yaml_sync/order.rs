//! `reorder`: the column items of an entry in the SQL's output order.
//! Each item moves whole — its lines, and the blank or comment lines
//! between it and the item before — so nothing inside one changes.

use cogitator_schemas::ColumnEntry;

use super::super::code_actions::yaml_text::is_content;
use super::fill::column_item_at;

/// The text with the items of `columns` ordered as `produced` lists
/// them (columns it does not list keep their order, last); `None` when
/// they already are, or the items are not a plain block-style list.
pub(super) fn reorder(text: &str, columns: &[ColumnEntry], produced: &[String]) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut items = Vec::with_capacity(columns.len());
    for column in columns {
        let entry = column_item_at(&lines, column.name_pos.line as usize).ok()?;
        let rank = produced
            .iter()
            .position(|p| p.eq_ignore_ascii_case(&column.name))
            .unwrap_or(usize::MAX);
        items.push((entry, rank));
    }
    items.sort_by_key(|(e, _)| e.dash_line);
    for pair in items.windows(2) {
        let between = pair[0].0.end + 1..pair[1].0.dash_line;
        if lines[between].iter().any(|l| is_content(l)) {
            return None; // something else sits between two items
        }
    }
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by_key(|&i| (items[i].1, i));
    if order.iter().enumerate().all(|(at, &i)| at == i) {
        return None;
    }
    let pieces: Vec<&str> = text.split_inclusive('\n').collect();
    let span = |i: usize| {
        let first = if i == 0 {
            items[0].0.dash_line
        } else {
            items[i - 1].0.end + 1
        };
        first..=items[i].0.end
    };
    let (first, last) = (items[0].0.dash_line, items[items.len() - 1].0.end);
    let mut out = String::with_capacity(text.len() + 1);
    for piece in &pieces[..first] {
        out.push_str(piece);
    }
    for &i in &order {
        for piece in &pieces[span(i)] {
            out.push_str(piece);
            if !piece.ends_with('\n') {
                out.push('\n');
            }
        }
    }
    for piece in &pieces[last + 1..] {
        out.push_str(piece);
    }
    if !text.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }
    Some(out)
}
