//! Document tokenization (specs/rust-port/02 behavior, 05 §4 mechanism,
//! 08 §1.1 zero-copy token model).
//!
//! No hand-written lexer: minijinja tokenizes the Jinja layer, sqlparser the
//! SQL layer; the refinement pass marks dbt constructs (REF/SOURCE/VAR/MACRO).

pub mod jinja_scan;
pub mod pipeline;

use std::collections::HashMap;
use std::sync::Arc;

use cogitator_dialects::Dialect;
use cogitator_error::Result;

pub use sqlparser::keywords::Keyword;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JinjaKind {
    Expr,
    Stmt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    /// SQL word: `Keyword::NoKeyword` means a plain identifier.
    Word(Keyword),
    /// Jinja-side identifier.
    Ident,
    // refined dbt kinds (02 §3.3)
    Ref,
    Var,
    SourceName,
    SourceTable,
    Macro,
    Package,
    Config,
    StringLit,
    Number,
    Symbol(char),
    JinjaOpen(JinjaKind),
    JinjaClose(JinjaKind),
    Other,
}

/// A token as a byte span into the document text (08 §1.1): no owned literal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    /// Byte span into `ParsedDocument::text`.
    pub start: u32,
    pub end: u32,
    /// 0-based line / byte column of `start` (Q6: byte-based, like Go).
    pub line: u32,
    pub column: u32,
}

impl Token {
    pub fn len(&self) -> u32 {
        self.end - self.start
    }
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// Byte-offset → (line, column) conversion for one document.
pub struct LineTable {
    /// Byte offset of each line start; `starts[0] == 0`.
    starts: Vec<u32>,
}

impl LineTable {
    pub fn new(text: &str) -> Self {
        let mut starts = vec![0u32];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                starts.push(i as u32 + 1);
            }
        }
        LineTable { starts }
    }

    pub fn line_col(&self, offset: u32) -> (u32, u32) {
        let line = match self.starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i - 1,
        };
        (line as u32, offset - self.starts[line])
    }

    pub fn line_start(&self, line: usize) -> Option<u32> {
        self.starts.get(line).copied()
    }
}

/// Fully tokenized document (02 §3.4–3.6, 08 §1.1).
#[derive(Debug, Clone, Default)]
pub struct ParsedDocument {
    pub text: Arc<str>,
    pub tokens: Vec<Token>,
    /// line → indices into `tokens`, ascending by column.
    line_index: HashMap<u32, Vec<u32>>,
    /// CTE name → definition token (02 §3.4; last definition wins).
    pub def_tokens: HashMap<Box<str>, Token>,
    /// SQL node structure from the tree-sitter backend (06 §3.6) —
    /// plain data so this crate stays free of tree-sitter; the phase-1
    /// pipeline leaves it `None` and features fall back to tokens.
    pub sql_structure: Option<SqlStructure>,
}

/// Named-SQL-node ranges in line/character space, document-wide.
#[derive(Debug, Clone, Default)]
pub struct SqlStructure {
    pub nodes: Vec<NodeRange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeRange {
    pub start_line: u32,
    pub start_character: u32,
    pub end_line: u32,
    pub end_character: u32,
}

impl NodeRange {
    /// Containment check in line/character space.
    pub fn contains(&self, line: u32, character: u32) -> bool {
        let after_start = line > self.start_line
            || (line == self.start_line && character >= self.start_character);
        let before_end =
            line < self.end_line || (line == self.end_line && character <= self.end_character);
        after_start && before_end
    }

    /// A coarse size metric for sorting containment chains: line span
    /// dominates; same-line ranges compare by character span.
    pub fn weight(&self) -> u64 {
        let line_span = u64::from(self.end_line.saturating_sub(self.start_line));
        let char_span = if self.start_line == self.end_line {
            u64::from(self.end_character.saturating_sub(self.start_character))
        } else {
            u64::from(self.end_character)
        };
        (line_span << 32) + char_span
    }
}

impl ParsedDocument {
    pub fn literal(&self, token: &Token) -> &str {
        &self.text[token.start as usize..token.end as usize]
    }

    /// Cursor lookup (02 §3.5): strictly inside the token's byte span.
    pub fn find_token_at_cursor(&self, line: u32, character: u32) -> Option<usize> {
        let indices = self.line_index.get(&line)?;
        let idx = indices.partition_point(|&i| {
            let t = &self.tokens[i as usize];
            t.column + t.len() <= character
        });
        let &token_index = indices.get(idx)?;
        let t = &self.tokens[token_index as usize];
        (character >= t.column && character < t.column + t.len()).then_some(token_index as usize)
    }

    /// Look back up to `max_steps` tokens for `kind` (02 §3.6 behavior; the
    /// crates-based token stream is denser than Go's, so this searches a
    /// window instead of demanding an exact distance).
    pub fn lookback_find(&self, idx: usize, kind: TokenKind, max_steps: usize) -> Option<&Token> {
        self.tokens[..idx]
            .iter()
            .rev()
            .take(max_steps)
            .find(|t| t.kind == kind)
    }

    pub(crate) fn build_indexes(&mut self) {
        let mut line_index: HashMap<u32, Vec<u32>> = HashMap::new();
        for (i, t) in self.tokens.iter().enumerate() {
            line_index.entry(t.line).or_default().push(i as u32);
        }
        for list in line_index.values_mut() {
            list.sort_by_key(|&i| self.tokens[i as usize].column);
        }
        self.line_index = line_index;
    }
}

/// The seam tree-sitter plugs into later (06 §3.5).
pub trait DocumentParser {
    /// Full parse. Errors mean "malformed mid-edit input" — the caller keeps
    /// serving the previous good `ParsedDocument` (05 §4 step 1).
    fn parse(&self, text: &str, dialect: &Dialect) -> Result<ParsedDocument>;
}

pub use pipeline::JinjaSqlParser;

/// Shared tokenization backend for alternative [`DocumentParser`]
/// implementations (06 §3.5 seam — used by the tree-sitter crate).
pub mod backend {
    pub use crate::pipeline::{
        assemble, collect_ctes, refine, sql_dialect, tokenize_sql_segment, tokenize_template,
    };
}
