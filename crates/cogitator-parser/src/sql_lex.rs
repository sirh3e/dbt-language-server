//! SQL-layer lexing (specs/rust-port/05 §4 step 2).
//!
//! The document is lexed once with every non-SQL byte blanked, so string
//! and comment state survives across Jinja: dbt renders the Jinja first,
//! which makes `'{{ x }}'` one string and `-- {{ x }} don't` one comment.
//! Blanks keep the byte length, so offsets map 1:1 onto the template.

use std::ops::Range;

use cogitator_dialects::Dialect;
use sqlparser::tokenizer::{Location, Token as SToken, TokenWithSpan, Tokenizer};

use crate::pipeline::{push, sql_dialect};
use crate::{JinjaKind, LineTable, Token, TokenKind};

/// Lex the SQL `segments` of `text` and merge the result with the `jinja`
/// tokens into one document-ordered stream.
///
/// Falls back to lexing each segment alone when the whole-document view
/// can't be trusted: mid-edit (unterminated string) or when a SQL token
/// would start in one Jinja branch and end in another.
pub fn lex_document(
    text: &str,
    segments: Vec<Range<usize>>,
    mut jinja: Vec<Token>,
    dialect: &Dialect,
    table: &LineTable,
) -> Vec<Token> {
    let segments = merge_adjacent(segments);
    jinja.sort_by_key(|t| t.start);

    let mut sql = Vec::new();
    if !lex_whole(text, &segments, &jinja, dialect, table, &mut sql) {
        sql.clear();
        for segment in &segments {
            lex_segment(
                &text[segment.clone()],
                segment.start,
                dialect,
                table,
                &mut sql,
            );
        }
    }

    jinja.extend(sql);
    jinja.sort_by_key(|t| t.start);
    jinja
}

/// Regions that touch are one run of SQL; a token may cross between them.
fn merge_adjacent(mut segments: Vec<Range<usize>>) -> Vec<Range<usize>> {
    segments.retain(|s| !s.is_empty());
    segments.sort_by_key(|s| s.start);
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(segments.len());
    for segment in segments {
        match merged.last_mut() {
            Some(last) if segment.start <= last.end => last.end = last.end.max(segment.end),
            _ => merged.push(segment),
        }
    }
    merged
}

/// Blank everything outside `segments`, newlines included: rendered Jinja
/// doesn't end a `--` comment.
fn masked_text(text: &str, segments: &[Range<usize>]) -> String {
    let mut masked = String::with_capacity(text.len());
    let mut pos = 0;
    for segment in segments {
        masked.extend(std::iter::repeat_n(' ', segment.start - pos));
        masked.push_str(&text[segment.clone()]);
        pos = segment.end;
    }
    masked.extend(std::iter::repeat_n(' ', text.len() - pos));
    masked
}

/// Returns false when the caller must fall back to per-segment lexing.
fn lex_whole(
    text: &str,
    segments: &[Range<usize>],
    jinja: &[Token],
    dialect: &Dialect,
    table: &LineTable,
    out: &mut Vec<Token>,
) -> bool {
    let masked = masked_text(text, segments);
    let Some(located) = tokenize(&masked, dialect) else {
        return false;
    };
    let branches = Branches::new(text, jinja);
    let mut cursor = Cursor::new(&masked);
    for tw in &located {
        let Some((start, end)) = cursor.span_of(tw) else {
            continue;
        };
        let Some(kind) = classify(&tw.token) else {
            continue;
        };
        if branches.at(start) != branches.at(end.saturating_sub(1)) {
            return false;
        }
        // A string around `{{ }}` overlaps Jinja tokens; those win.
        if within_one_segment(segments, start, end) {
            push(out, kind, start as u32, end as u32, table);
        }
    }
    true
}

fn within_one_segment(segments: &[Range<usize>], start: usize, end: usize) -> bool {
    let i = segments.partition_point(|s| s.end <= start);
    segments
        .get(i)
        .is_some_and(|s| s.start <= start && end <= s.end)
}

/// Lex one SQL segment on its own, appending document-absolute tokens. A
/// tokenizer error skips the segment rather than failing the parse.
fn lex_segment(
    segment: &str,
    base: usize,
    dialect: &Dialect,
    table: &LineTable,
    out: &mut Vec<Token>,
) {
    let Some(located) = tokenize(segment, dialect) else {
        return;
    };
    let mut cursor = Cursor::new(segment);
    for tw in &located {
        let Some((start, end)) = cursor.span_of(tw) else {
            continue;
        };
        if let Some(kind) = classify(&tw.token) {
            push(out, kind, (base + start) as u32, (base + end) as u32, table);
        }
    }
}

fn tokenize(sql: &str, dialect: &Dialect) -> Option<Vec<TokenWithSpan>> {
    let dialect = sql_dialect(dialect);
    match Tokenizer::new(dialect.as_ref(), sql).tokenize_with_location() {
        Ok(located) => Some(located),
        Err(err) => {
            tracing::trace!(%err, "sql tokenize failed");
            None
        }
    }
}

/// The refined kind of a located token; `None` for whitespace and comments.
fn classify(token: &SToken) -> Option<TokenKind> {
    let kind = match token {
        SToken::Whitespace(_) | SToken::EOF => return None,
        SToken::Word(w) => TokenKind::Word(w.keyword),
        SToken::Number(..) => TokenKind::Number,
        SToken::SingleQuotedString(_) | SToken::DoubleQuotedString(_) => TokenKind::StringLit,
        SToken::LParen => TokenKind::Symbol('('),
        SToken::RParen => TokenKind::Symbol(')'),
        SToken::Comma => TokenKind::Symbol(','),
        SToken::Period => TokenKind::Symbol('.'),
        SToken::SemiColon => TokenKind::Symbol(';'),
        other => TokenKind::Symbol(other.to_string().chars().next().unwrap_or(' ')),
    };
    Some(kind)
}

/// Converts sqlparser's 1-based (line, char column) locations to byte
/// offsets. Locations arrive in document order, so one forward walk serves
/// them all; columns count chars, which is why they can't be added to bytes.
struct Cursor<'a> {
    rest: std::str::Chars<'a>,
    byte: usize,
    line: u64,
    column: u64,
}

impl<'a> Cursor<'a> {
    fn new(text: &'a str) -> Self {
        Cursor {
            rest: text.chars(),
            byte: 0,
            line: 1,
            column: 1,
        }
    }

    /// Advance to `to`. A location behind the cursor stays at the cursor.
    fn seek(&mut self, to: Location) -> usize {
        while (self.line, self.column) < (to.line, to.column) {
            let Some(c) = self.rest.next() else { break };
            self.byte += c.len_utf8();
            if c == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
        }
        self.byte
    }

    /// Byte range of a located token; line 0 marks an empty span.
    fn span_of(&mut self, tw: &TokenWithSpan) -> Option<(usize, usize)> {
        if tw.span.start.line == 0 {
            return None;
        }
        let start = self.seek(tw.span.start);
        Some((start, self.seek(tw.span.end)))
    }
}

/// Which Jinja control branch each offset belongs to.
struct Branches {
    /// (offset the branch starts at, branch id), ascending.
    marks: Vec<(usize, u32)>,
}

impl Branches {
    fn new(text: &str, jinja: &[Token]) -> Self {
        let mut marks = vec![(0, 0)];
        let mut stack = vec![0u32];
        let mut next_id = 1;
        for (i, token) in jinja.iter().enumerate() {
            if token.kind != TokenKind::JinjaOpen(JinjaKind::Stmt) {
                continue;
            }
            let Some(tag) = jinja.get(i + 1) else { break };
            let tag = &text[tag.start as usize..tag.end as usize];
            match tag {
                "if" | "for" => stack.push(next_id),
                "elif" | "else" => {
                    stack.pop();
                    stack.push(next_id);
                }
                "endif" | "endfor" if stack.len() > 1 => {
                    stack.pop();
                }
                _ => continue,
            }
            next_id += 1;
            let at = token.start as usize;
            marks.push((at, *stack.last().unwrap_or(&0)));
        }
        Branches { marks }
    }

    fn at(&self, offset: usize) -> u32 {
        let i = self.marks.partition_point(|&(at, _)| at <= offset);
        self.marks[i.saturating_sub(1)].1
    }
}
