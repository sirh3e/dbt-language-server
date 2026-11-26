//! The cursor's SQL as sqlparser reads it: the template with its Jinja
//! masked in place (`{{ ref('x') }}` an identifier of equal length,
//! control flow flattened), a placeholder identifier at the cursor, and
//! the variants of the cursor's query that a half-typed statement still
//! parses as. Every variant keeps the document's lines and columns (what
//! it leaves out is blanked, never removed), so a sqlparser span maps
//! straight back to a document offset.

use std::borrow::Cow;

use cogitator_dialects::Dialect;
use cogitator_parser::ParsedDocument;
use sqlparser::dialect::Dialect as SqlDialect;
use sqlparser::keywords::Keyword;
use sqlparser::tokenizer::{Location, Token, Tokenizer};

use crate::Document;

/// The identifier written at the cursor. Whatever clause holds it after
/// parsing is the cursor's clause.
pub(super) const PH: &str = "__cogitator_cursor__";

/// The parse a document's Jinja masking is read from: the stored one
/// when it is of the current text, a fresh one otherwise (the store keeps
/// the last good tokens of a buffer that stopped tokenizing).
pub(super) fn current_parse<'d>(
    doc: &'d Document,
    dialect: &Dialect,
) -> Option<Cow<'d, ParsedDocument>> {
    if *doc.parsed.text == *doc.text {
        return Some(Cow::Borrowed(&doc.parsed));
    }
    cogitator_parser::pipeline::parse_document(&doc.text, dialect)
        .ok()
        .map(Cow::Owned)
}

/// The template as SQL of the same length: `cogitator_duckdb`'s masking,
/// after flattening `if`/`for` with the local-rename masking when the
/// template has control flow.
pub(super) fn masked(parsed: &ParsedDocument, dialect: &Dialect) -> Option<String> {
    if let Some(sql) = cogitator_duckdb::mask_jinja(parsed) {
        return Some(sql);
    }
    let flat = crate::features::rename_local::sql::mask::without_control_tags(&parsed.text)?;
    let flat = cogitator_parser::pipeline::parse_document(&flat, dialect).ok()?;
    cogitator_duckdb::mask_jinja(&flat)
}

/// The masked SQL with the placeholder at the cursor.
pub(super) struct Text {
    pub(super) text: String,
    /// Byte offset of the placeholder.
    pub(super) cursor: usize,
    line_starts: Vec<usize>,
}

/// A token of [`Text`] by its byte offset.
#[derive(Debug, Clone)]
pub(super) struct Tok {
    pub(super) token: Token,
    pub(super) start: usize,
    pub(super) end: usize,
}

impl Text {
    pub(super) fn new(masked: &str, cursor: usize) -> Self {
        let cursor = (0..=cursor.min(masked.len()))
            .rev()
            .find(|&i| masked.is_char_boundary(i))
            .unwrap_or(0);
        let text = format!("{}{PH}{}", &masked[..cursor], &masked[cursor..]);
        let line_starts = std::iter::once(0)
            .chain(text.match_indices('\n').map(|(i, _)| i + 1))
            .collect();
        Text {
            text,
            cursor,
            line_starts,
        }
    }

    pub(super) fn ph_end(&self) -> usize {
        self.cursor + PH.len()
    }

    /// The byte offset of a sqlparser location (1-based, columns counted
    /// in characters).
    pub(super) fn offset(&self, loc: Location) -> Option<usize> {
        let line_start = *self.line_starts.get((loc.line as usize).checked_sub(1)?)?;
        let line = &self.text[line_start..];
        let column = (loc.column as usize).checked_sub(1)?;
        line.char_indices()
            .nth(column)
            .map(|(i, _)| line_start + i)
            .or_else(|| (line.chars().count() == column).then(|| line_start + line.len()))
    }

    /// The document offset of a byte offset of this text.
    pub(super) fn doc_offset(&self, offset: usize) -> usize {
        if offset >= self.ph_end() {
            offset - PH.len()
        } else {
            offset.min(self.cursor)
        }
    }

    /// The text's tokens, without white space and comments.
    pub(super) fn tokens(&self, dialect: &dyn SqlDialect) -> Option<Vec<Tok>> {
        let tokens = Tokenizer::new(dialect, &self.text)
            .with_unescape(false)
            .tokenize_with_location()
            .ok()?;
        tokens
            .into_iter()
            .filter(|t| !matches!(t.token, Token::Whitespace(_)))
            .map(|t| {
                Some(Tok {
                    start: self.offset(t.span.start)?,
                    end: self.offset(t.span.end).unwrap_or(self.text.len()),
                    token: t.token,
                })
            })
            .collect()
    }

    /// `self.text` with only `keep` (byte ranges) left, the rest blanked
    /// to spaces (line breaks stay, so locations do not move).
    pub(super) fn keeping(&self, keep: &[(usize, usize)]) -> String {
        let mut out: Vec<u8> = self.text.as_bytes().to_vec();
        let kept = |i: usize| keep.iter().any(|&(lo, hi)| lo <= i && i < hi);
        for (i, b) in out.iter_mut().enumerate() {
            if !kept(i) && !matches!(b, b'\n' | b'\r') {
                *b = b' ';
            }
        }
        // blanking whole bytes can split a character; such a byte is
        // never in a kept range's middle, so the lossy repair only
        // touches blanked space
        String::from_utf8(out)
            .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
    }
}

/// The statement around the cursor: `;`-separated, at depth 0.
pub(super) fn statement(tokens: &[Tok], text: &Text) -> (usize, usize) {
    let (mut lo, mut hi) = (0, text.text.len());
    let mut depth = 0i32;
    for t in tokens {
        match t.token {
            Token::LParen => depth += 1,
            Token::RParen => depth -= 1,
            Token::SemiColon if depth <= 0 => {
                if t.end <= text.cursor {
                    lo = t.end;
                } else if t.start >= text.ph_end() {
                    hi = t.start;
                    break;
                }
            }
            _ => {}
        }
    }
    (lo, hi)
}

/// Every parenthesized query: `(` followed by SELECT or WITH, up to its
/// matching `)` (exclusive both ends), innermost last among the ones that
/// contain a given offset.
pub(super) fn query_regions(tokens: &[Tok]) -> Vec<(usize, usize)> {
    let mut open: Vec<(usize, bool)> = Vec::new();
    let mut regions = Vec::new();
    for (i, t) in tokens.iter().enumerate() {
        match t.token {
            Token::LParen => {
                let query = tokens.get(i + 1).is_some_and(|n| {
                    matches!(&n.token, Token::Word(w) if matches!(w.keyword, Keyword::SELECT | Keyword::WITH))
                });
                open.push((t.end, query));
            }
            Token::RParen => {
                if let Some((start, true)) = open.pop() {
                    regions.push((start, t.start));
                }
            }
            _ => {}
        }
    }
    regions.sort_unstable();
    regions
}

/// The innermost of `regions` holding `[lo, hi)`, else `outer`.
pub(super) fn innermost(
    regions: &[(usize, usize)],
    lo: usize,
    hi: usize,
    outer: (usize, usize),
) -> (usize, usize) {
    regions
        .iter()
        .filter(|&&(a, b)| a <= lo && hi <= b)
        .min_by_key(|&&(a, b)| b - a)
        .copied()
        .unwrap_or(outer)
}

/// Where the clauses of a query region start, at its own depth: what a
/// broken clause after the cursor is cut back to.
pub(super) fn clause_starts(tokens: &[Tok], region: (usize, usize)) -> Vec<usize> {
    let mut depth = 0i32;
    let mut starts = Vec::new();
    let mut previous_joins = false;
    for t in tokens
        .iter()
        .filter(|t| region.0 <= t.start && t.end <= region.1)
    {
        match &t.token {
            Token::LParen => depth += 1,
            Token::RParen => depth -= 1,
            Token::Word(w) if depth == 0 => {
                let joining = matches!(
                    w.keyword,
                    Keyword::JOIN
                        | Keyword::LEFT
                        | Keyword::RIGHT
                        | Keyword::INNER
                        | Keyword::FULL
                        | Keyword::CROSS
                        | Keyword::OUTER
                        | Keyword::NATURAL
                );
                let clause = matches!(
                    w.keyword,
                    Keyword::FROM
                        | Keyword::WHERE
                        | Keyword::GROUP
                        | Keyword::HAVING
                        | Keyword::QUALIFY
                        | Keyword::WINDOW
                        | Keyword::ORDER
                        | Keyword::LIMIT
                        | Keyword::UNION
                        | Keyword::EXCEPT
                        | Keyword::INTERSECT
                ) || (joining && !previous_joins);
                if clause {
                    starts.push(t.start);
                }
                previous_joins = joining;
                continue;
            }
            _ => {}
        }
        previous_joins = false;
    }
    starts
}

/// What to try parsing for a query region, best first: the whole region;
/// then the region up to the placeholder followed by the clauses after
/// it, cut back one clause at a time; then the region up to the
/// placeholder alone.
pub(super) fn variants(text: &Text, tokens: &[Tok], region: (usize, usize)) -> Vec<String> {
    let mut out = vec![text.keeping(&[region])];
    let end = text.ph_end();
    if !(region.0 <= text.cursor && end <= region.1) {
        return out;
    }
    let after: Vec<usize> = clause_starts(tokens, region)
        .into_iter()
        .filter(|&s| s >= end)
        .collect();
    if let Some(&first) = after.first() {
        let ends = after
            .iter()
            .skip(1)
            .copied()
            .chain(std::iter::once(region.1));
        let mut cuts: Vec<usize> = ends.collect();
        cuts.reverse();
        for cut in cuts {
            out.push(text.keeping(&[(region.0, end), (first, cut)]));
        }
    }
    out.push(text.keeping(&[(region.0, end)]));
    out
}
