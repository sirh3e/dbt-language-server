//! textDocument/linkedEditingRange (01 §6.3): rename-in-place for CTE
//! names — the cursor token must be a CTE the document itself defines,
//! and every same-kind occurrence edits together.

use cogitator_lsp::{LinkedEditingRanges, Position};

use super::{same_literal_ranges, Document};

pub fn linked_editing(doc: &Document, pos: Position) -> Option<LinkedEditingRanges> {
    let idx = doc.parsed.find_token_at_cursor(pos.line, pos.character)?;
    let token = &doc.parsed.tokens[idx];
    let literal = doc.parsed.literal(token);
    // only CTE names: linked editing of arbitrary identifiers would
    // happily rewrite column names that merely coincide
    if !doc.parsed.def_tokens.contains_key(literal) {
        return None;
    }
    let ranges = same_literal_ranges(doc, token);
    (ranges.len() > 1).then_some(LinkedEditingRanges {
        ranges,
        word_pattern: None,
    })
}
