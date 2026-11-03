//! SQL structure via the tree-sitter-sequel grammar (06 §3.6): the
//! document is masked (Jinja regions blanked, newlines kept, so every
//! node position stays document-accurate), parsed once, and the named
//! nodes become plain `NodeRange`s for folding and selection ranges.
//! SQL *tokens* stay on sqlparser (05 §4) — this is structure only.

use std::sync::Mutex;

use cogitator_parser::{NodeRange, SqlStructure};

use crate::Region;

/// Hard cap so a pathological document cannot balloon the parsed state.
const MAX_NODES: usize = 4096;

pub(crate) fn new_sql_parser() -> Option<Mutex<tree_sitter::Parser>> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_sequel::LANGUAGE.into())
        .ok()?;
    Some(Mutex::new(parser))
}

/// Blank every Jinja/comment/error region (non-newline bytes → spaces) so
/// the SQL grammar sees pure SQL at unchanged positions.
fn mask(text: &str, regions: &[Region]) -> String {
    let mut bytes = text.as_bytes().to_vec();
    for region in regions {
        let end = region.end.min(bytes.len());
        for byte in &mut bytes[region.start..end] {
            if *byte != b'\n' && *byte != b'\r' {
                *byte = b' ';
            }
        }
    }
    // regions sit on char boundaries, so this stays valid UTF-8; be
    // defensive anyway rather than panicking on a grammar surprise
    String::from_utf8(bytes).unwrap_or_else(|_| text.to_string())
}

/// Parse the masked document and collect named-node ranges (deduped,
/// capped). `None` when the parser is unavailable or produced nothing.
pub(crate) fn collect(
    parser: &Option<Mutex<tree_sitter::Parser>>,
    text: &str,
    regions: &[Region],
) -> Option<SqlStructure> {
    let parser = parser.as_ref()?;
    let masked = mask(text, regions);
    let tree = parser.lock().ok()?.parse(&masked, None)?;
    let mut nodes = Vec::new();
    let mut cursor = tree.root_node().walk();
    let mut reached_root = false;
    while !reached_root && nodes.len() < MAX_NODES {
        let node = cursor.node();
        if node.is_named() && node.byte_range().len() > 1 {
            let start = node.start_position();
            let end = node.end_position();
            nodes.push(NodeRange {
                start_line: start.row as u32,
                start_character: start.column as u32,
                end_line: end.row as u32,
                end_character: end.column as u32,
            });
        }
        if cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                reached_root = true;
                break;
            }
        }
    }
    nodes.sort_by_key(|n| (n.start_line, n.start_character, n.end_line, n.end_character));
    nodes.dedup();
    if nodes.is_empty() {
        return None;
    }
    Some(SqlStructure { nodes })
}
