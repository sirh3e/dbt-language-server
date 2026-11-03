//! Tree-sitter document backend (specs/rust-port/06 §3): error-tolerant
//! segmentation of dbt SQL-with-Jinja via the vendored dbt-labs
//! tree-sitter-jinja2 grammar, behind the `DocumentParser` seam.
//!
//! Division of labor: the tree decides *where* the Jinja constructs are —
//! and keeps deciding it on broken mid-edit buffers (ERROR nodes degrade one
//! region, not the document). Each well-formed construct is then tokenized
//! by minijinja and the gaps by sqlparser through `cogitator_parser::backend`, so
//! refinement, CTE tracking and the token model stay byte-for-byte identical
//! to the phase-1 pipeline. SQL stays on sqlparser per 06 §3.3.

mod ffi;
mod sql_structure;

use std::collections::HashMap;
use std::sync::Mutex;

use cogitator_dialects::Dialect;
use cogitator_error::Result;
use cogitator_parser::{backend, AppliedEdit, DocumentParser, LineTable, ParsedDocument, Token};

/// Below this size a full reparse is µs-scale anyway; the incremental
/// path (06 §3.4) only engages past it — risk stays bounded.
const INCREMENTAL_MIN_BYTES: usize = 32 * 1024;

/// Phase-2 `DocumentParser` (06 §3.5). Unlike `JinjaSqlParser`, `parse`
/// practically never fails: malformed regions contribute no tokens instead
/// of staling the whole document.
pub struct TreeSitterParser {
    parser: Mutex<tree_sitter::Parser>,
    /// tree-sitter-sequel for SQL *structure* only (06 §3.6); `None`
    /// (grammar failed to load) just means token-based fallbacks.
    sql_parser: Option<Mutex<tree_sitter::Parser>>,
    /// Last tree per open document (06 §3.4) — fed back as the old
    /// tree on big ranged edits, dropped on didClose via `forget`.
    trees: Mutex<HashMap<String, tree_sitter::Tree>>,
}

impl TreeSitterParser {
    pub fn new() -> Result<Self> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&ffi::language())
            .map_err(|e| cogitator_error::Error::Jinja {
                path: Default::default(),
                message: format!("tree-sitter language load failed: {e}"),
            })?;
        Ok(TreeSitterParser {
            parser: Mutex::new(parser),
            sql_parser: sql_structure::new_sql_parser(),
            trees: Mutex::new(HashMap::new()),
        })
    }
}

impl TreeSitterParser {
    /// The shared tail of every parse: tokens from the tree, SQL
    /// structure from tree-sitter-sequel.
    fn document_from(
        &self,
        tree: &tree_sitter::Tree,
        text: &str,
        dialect: &Dialect,
    ) -> ParsedDocument {
        let mut doc = parse_with_tree(tree, text, dialect);
        let regions = collect_regions(tree.root_node(), text);
        doc.sql_structure = sql_structure::collect(&self.sql_parser, text, &regions);
        doc
    }
}

impl DocumentParser for TreeSitterParser {
    fn parse(&self, text: &str, dialect: &Dialect) -> Result<ParsedDocument> {
        let tree = self
            .parser
            .lock()
            .ok()
            .and_then(|mut parser| parser.parse(text, None));
        match tree {
            Some(tree) => Ok(self.document_from(&tree, text, dialect)),
            // parse() returns None only on cancellation/timeout; fall back to
            // the phase-1 pipeline rather than dropping the request.
            None => backend_parse(text, dialect),
        }
    }

    /// 06 §3.4: feed the previous tree + InputEdits back into
    /// tree-sitter for documents past the size gate; everything else is
    /// a full parse. Either way the fresh tree is remembered per uri.
    fn parse_incremental(
        &self,
        uri: &str,
        text: &str,
        dialect: &Dialect,
        edits: &[AppliedEdit],
    ) -> Result<ParsedDocument> {
        let old_tree = if !edits.is_empty() && text.len() > INCREMENTAL_MIN_BYTES {
            self.trees
                .lock()
                .ok()
                .and_then(|trees| trees.get(uri).cloned())
                .map(|mut tree| {
                    for edit in edits {
                        tree.edit(&input_edit(edit));
                    }
                    tree
                })
        } else {
            None
        };
        let tree = self
            .parser
            .lock()
            .ok()
            .and_then(|mut parser| parser.parse(text, old_tree.as_ref()));
        match tree {
            Some(tree) => {
                let doc = self.document_from(&tree, text, dialect);
                if let Ok(mut trees) = self.trees.lock() {
                    trees.insert(uri.to_string(), tree);
                }
                Ok(doc)
            }
            None => backend_parse(text, dialect),
        }
    }

    fn forget(&self, uri: &str) {
        if let Ok(mut trees) = self.trees.lock() {
            trees.remove(uri);
        }
    }
}

fn input_edit(edit: &AppliedEdit) -> tree_sitter::InputEdit {
    let point = |(row, column): (u32, u32)| tree_sitter::Point {
        row: row as usize,
        column: column as usize,
    };
    tree_sitter::InputEdit {
        start_byte: edit.start_byte,
        old_end_byte: edit.old_end_byte,
        new_end_byte: edit.new_end_byte,
        start_position: point(edit.start_point),
        old_end_position: point(edit.old_end_point),
        new_end_position: point(edit.new_end_point),
    }
}

fn backend_parse(text: &str, dialect: &Dialect) -> Result<ParsedDocument> {
    cogitator_parser::pipeline::parse_document(text, dialect)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegionKind {
    Jinja,
    Comment,
    Error,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Region {
    kind: RegionKind,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

fn parse_with_tree(tree: &tree_sitter::Tree, text: &str, dialect: &Dialect) -> ParsedDocument {
    let regions = collect_regions(tree.root_node(), text);
    let table = LineTable::new(text);
    let mut tokens: Vec<Token> = Vec::new();

    let mut pos = 0usize;
    for region in &regions {
        if region.start > pos {
            backend::tokenize_sql_segment(
                &text[pos..region.start],
                pos as u32,
                dialect,
                &table,
                &mut tokens,
            );
        }
        let slice = &text[region.start..region.end];
        match region.kind {
            RegionKind::Comment => {}
            RegionKind::Jinja => tokenize_region(slice, region.start, dialect, &table, &mut tokens),
            RegionKind::Error => {
                // A broken region that still looks like Jinja degrades to
                // nothing (06 §3.3); anything else is SQL the grammar
                // tripped over.
                if slice.contains("{{") || slice.contains("{%") || slice.contains("{#") {
                    tokenize_region(slice, region.start, dialect, &table, &mut tokens);
                } else {
                    backend::tokenize_sql_segment(
                        slice,
                        region.start as u32,
                        dialect,
                        &table,
                        &mut tokens,
                    );
                }
            }
        }
        pos = region.end.max(pos);
    }
    if pos < text.len() {
        backend::tokenize_sql_segment(&text[pos..], pos as u32, dialect, &table, &mut tokens);
    }

    backend::assemble(text, tokens)
}

/// Tokenize one Jinja region with minijinja; a malformed region contributes
/// no tokens instead of failing the document.
fn tokenize_region(
    slice: &str,
    base: usize,
    dialect: &Dialect,
    table: &LineTable,
    out: &mut Vec<Token>,
) {
    let mut region_tokens = Vec::new();
    match backend::tokenize_template(slice, base as u32, dialect, table, &mut region_tokens) {
        Ok(()) => out.extend(region_tokens),
        Err(err) => tracing::trace!(%err, base, "malformed jinja region skipped"),
    }
}

/// Top-level walk of the jinja2 CST: group `{{ … }}` token runs, take
/// `jinja_expression` ({% … %}) and comment spans, and surface ERROR spans.
/// Everything between regions is SQL text.
fn collect_regions(root: tree_sitter::Node<'_>, text: &str) -> Vec<Region> {
    let mut regions = Vec::new();
    let count = root.child_count();
    let mut i = 0;
    while i < count {
        let Some(node) = root.child(i) else { break };
        match node.kind() {
            "{{" => {
                // run to the matching `}}` sibling (or as far as the children go)
                let start = node.start_byte();
                let mut end = node.end_byte();
                let mut j = i + 1;
                while let Some(sibling) = root.child(j) {
                    end = sibling.end_byte();
                    if sibling.kind() == "}}" {
                        break;
                    }
                    j += 1;
                }
                regions.push(Region {
                    kind: RegionKind::Jinja,
                    start,
                    end,
                });
                i = j;
            }
            "jinja_expression" => {
                regions.push(Region {
                    kind: RegionKind::Jinja,
                    start: node.start_byte(),
                    end: node.end_byte(),
                });
            }
            "{#" => {
                // The grammar marks the comment-body token HIDDEN, so the
                // sibling walk can land on a later construct and swallow
                // it (a well-formed comment then ate the rest of the
                // document). Find the real `#}` in the text instead; an
                // unterminated comment still runs to the end.
                let start = node.start_byte();
                let end = text[start + 2..]
                    .find("#}")
                    .map(|off| start + 2 + off + 2)
                    .unwrap_or(text.len());
                regions.push(Region {
                    kind: RegionKind::Comment,
                    start,
                    end,
                });
            }
            "ERROR" => {
                regions.push(Region {
                    kind: RegionKind::Error,
                    start: node.start_byte(),
                    end: node.end_byte(),
                });
            }
            _ => {} // anonymous text tokens → SQL gap
        }
        i += 1;
    }
    // defensive: sorted, non-overlapping
    regions.sort_by_key(|r| r.start);
    regions.dedup_by(|b, a| {
        if b.start < a.end {
            a.end = a.end.max(b.end);
            true
        } else {
            false
        }
    });
    regions
}
