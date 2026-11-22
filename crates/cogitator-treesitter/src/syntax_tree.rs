//! The tree-sitter-dbt syntax tree of a document as plain data, for
//! `dbt.syntaxTree` (an editor's tree inspector). Pure: text in, nodes
//! out in pre-order with their depth, so a client draws the tree as an
//! indented list and maps its cursor through the ranges.

/// A position in LSP terms: 0-based line, UTF-16 code units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub line: u32,
    pub character: u32,
}

/// One named node (or an ERROR / MISSING node) of the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxNode {
    /// 0 for the root.
    pub depth: u32,
    pub kind: String,
    /// The field name under its parent, when the grammar gives one.
    pub field: Option<String>,
    pub error: bool,
    pub missing: bool,
    pub start: Point,
    pub end: Point,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyntaxTree {
    pub nodes: Vec<SyntaxNode>,
    /// More nodes than `max_nodes`; the rest are left out.
    pub truncated: bool,
}

/// UTF-16 column of a byte column within one line of `text`.
fn utf16_column(lines: &[&str], row: usize, byte_column: usize) -> u32 {
    let line = lines.get(row).copied().unwrap_or("");
    let prefix = line.get(..byte_column.min(line.len())).unwrap_or(line);
    prefix.encode_utf16().count() as u32
}

/// Parse `text` with tree-sitter-dbt and list its named nodes, at most
/// `max_nodes` of them. `None` when the grammar cannot be loaded.
pub fn syntax_tree(text: &str, max_nodes: usize) -> Option<SyntaxTree> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_dbt::LANGUAGE.into())
        .ok()?;
    let tree = parser.parse(text, None)?;
    let lines: Vec<&str> = text.split('\n').collect();
    let point = |p: tree_sitter::Point| Point {
        line: p.row as u32,
        character: utf16_column(&lines, p.row, p.column),
    };

    let mut out = SyntaxTree::default();
    let mut cursor = tree.walk();
    let mut depth: u32 = 0;
    loop {
        let node = cursor.node();
        if node.is_named() || node.is_error() || node.is_missing() {
            if out.nodes.len() == max_nodes {
                out.truncated = true;
                break;
            }
            out.nodes.push(SyntaxNode {
                depth,
                kind: node.kind().to_string(),
                field: cursor.field_name().map(str::to_string),
                error: node.is_error(),
                missing: node.is_missing(),
                start: point(node.start_position()),
                end: point(node.end_position()),
            });
        }
        if cursor.goto_first_child() {
            depth += 1;
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return Some(out);
            }
            depth -= 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn lists_named_nodes_in_pre_order_with_depths() {
        let tree = syntax_tree("select * from {{ ref('a') }}\n", 1000).expect("grammar loads");
        assert!(!tree.truncated);
        let root = &tree.nodes[0];
        assert_eq!(root.depth, 0);
        assert_eq!(
            root.start,
            Point {
                line: 0,
                character: 0
            }
        );
        // every node after the root is deeper than it and nested inside it
        assert!(tree.nodes[1..].iter().all(|n| n.depth >= 1));
        // pre-order: depth grows by at most one from one node to the next
        assert!(tree.nodes.windows(2).all(|w| w[1].depth <= w[0].depth + 1));
        assert!(
            tree.nodes
                .iter()
                .any(|n| n.kind.contains("expression") || n.kind.contains("call")),
            "the {{ ref() }} shows up as a node: {:?}",
            tree.nodes.iter().map(|n| &n.kind).collect::<Vec<_>>()
        );
    }

    #[test]
    fn columns_are_utf16_code_units() {
        // "é" is two bytes but one UTF-16 unit; "😀" is four bytes, two units
        let text = "{# é😀 #}{{ x }}\n";
        let tree = syntax_tree(text, 1000).expect("grammar loads");
        let comment = tree
            .nodes
            .iter()
            .find(|n| n.kind == "comment")
            .expect("a comment node");
        let output = tree
            .nodes
            .iter()
            .find(|n| n.kind == "output")
            .expect("an output node");
        assert_eq!(
            comment.end,
            Point {
                line: 0,
                character: 9
            }
        );
        assert_eq!(
            output.start,
            Point {
                line: 0,
                character: 9
            }
        );
        assert_eq!("{# é😀 #}".len(), 12, "the byte column would differ");
    }

    #[test]
    fn stops_at_the_cap_and_says_so() {
        let text = "{{ a }} {{ b }} {{ c }} {{ d }}\n";
        let tree = syntax_tree(text, 2).expect("grammar loads");
        assert_eq!(tree.nodes.len(), 2);
        assert!(tree.truncated);
    }

    #[test]
    fn broken_templates_keep_their_error_nodes() {
        let tree = syntax_tree("select {{ ref('a' \n", 1000).expect("grammar loads");
        assert!(tree.nodes.iter().any(|n| n.error || n.missing));
    }
}
