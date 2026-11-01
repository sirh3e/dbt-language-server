//! Folding, selection ranges, semantic tokens (token-based v1, 06 §3.6).

use dbt_ls_lsp::{
    FoldingRange, Position, Range, SelectionRange, SemanticTokens, SEMANTIC_TOKEN_TYPES,
};
use dbt_ls_parser::{Keyword, TokenKind};

use super::{token_range, Document};

/// textDocument/foldingRange: multi-line paren pairs (CTE bodies) — token-
/// based v1; the tree-sitter phase upgrades this (06 §3.6).
pub fn folding_ranges(doc: &Document) -> Vec<FoldingRange> {
    let mut stack: Vec<u32> = Vec::new();
    let mut out = Vec::new();
    for t in &doc.parsed.tokens {
        match t.kind {
            TokenKind::Symbol('(') => stack.push(t.line),
            TokenKind::Symbol(')') => {
                if let Some(open_line) = stack.pop() {
                    if t.line > open_line {
                        out.push(FoldingRange {
                            start_line: open_line,
                            end_line: t.line,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    out.sort_by_key(|r| (r.start_line, r.end_line));
    out.dedup_by_key(|r| (r.start_line, r.end_line));
    out
}

/// textDocument/selectionRange: token → line → document chain per position.
pub fn selection_ranges(doc: &Document, positions: &[Position]) -> Vec<SelectionRange> {
    let line_count = doc.text.split('\n').count() as u32;
    positions
        .iter()
        .map(|pos| {
            let document = SelectionRange {
                range: Range {
                    start: Position::default(),
                    end: Position {
                        line: line_count.saturating_sub(1),
                        character: 0,
                    },
                },
                parent: None,
            };
            let line_len = doc
                .text
                .split('\n')
                .nth(pos.line as usize)
                .map(|l| l.len() as u32)
                .unwrap_or(0);
            let line = SelectionRange {
                range: Range {
                    start: Position {
                        line: pos.line,
                        character: 0,
                    },
                    end: Position {
                        line: pos.line,
                        character: line_len,
                    },
                },
                parent: Some(Box::new(document)),
            };
            match doc.parsed.find_token_at_cursor(pos.line, pos.character) {
                Some(idx) => SelectionRange {
                    range: token_range(&doc.parsed.tokens[idx]),
                    parent: Some(Box::new(line)),
                },
                None => line,
            }
        })
        .collect()
}

/// textDocument/semanticTokens/full: LSP delta encoding over the refined
/// token stream (token-based v1, legend in dbt_ls_lsp::SEMANTIC_TOKEN_TYPES).
pub fn semantic_tokens(doc: &Document) -> SemanticTokens {
    let type_index = |name: &str| {
        SEMANTIC_TOKEN_TYPES
            .iter()
            .position(|t| *t == name)
            .unwrap() as u32
    };
    let keyword = type_index("keyword");
    let macro_t = type_index("macro");
    let variable = type_index("variable");
    let string_t = type_index("string");
    let number = type_index("number");
    let namespace = type_index("namespace");

    let mut data = Vec::new();
    let (mut prev_line, mut prev_col) = (0u32, 0u32);
    for t in &doc.parsed.tokens {
        let token_type = match t.kind {
            TokenKind::Word(kw) if kw != Keyword::NoKeyword => keyword,
            TokenKind::Config => keyword,
            TokenKind::Macro => macro_t,
            TokenKind::Package => namespace,
            TokenKind::Ref | TokenKind::SourceName | TokenKind::SourceTable | TokenKind::Var => {
                variable
            }
            TokenKind::StringLit => string_t,
            TokenKind::Number => number,
            _ => continue,
        };
        if doc.parsed.literal(t).contains('\n') {
            continue; // single-line encoding only
        }
        let delta_line = t.line - prev_line;
        let delta_col = if delta_line == 0 {
            t.column - prev_col
        } else {
            t.column
        };
        data.extend_from_slice(&[delta_line, delta_col, t.len(), token_type, 0]);
        (prev_line, prev_col) = (t.line, t.column);
    }
    SemanticTokens { data }
}
