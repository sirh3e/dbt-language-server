//! A document into its pieces, on minijinja's own tokens: the SQL
//! between tags, `{{ … }}` expressions, `{% … %}` statements,
//! `{# … #}` comments and `{% raw %} … {% endraw %}` regions.
//!
//! minijinja's tokenizer yields no token for a comment, a raw tag or
//! whitespace a `-` trims: those are the gaps between token spans, and
//! the gap is read here, so every byte of the source lands in exactly
//! one piece and the pieces concatenate back to the source.

use minijinja::machinery::{tokenize, Token};
use minijinja::syntax::SyntaxConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Sql,
    /// `{{ … }}`.
    Expr,
    /// `{% … %}`.
    Stmt,
    /// `{# … #}`.
    Comment,
    /// `{% raw %} … {% endraw %}`, kept byte for byte.
    Raw,
}

/// One token inside a tag: what it is and its text in the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tok<'a> {
    pub kind: TokKind,
    pub text: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokKind {
    Ident,
    Str,
    Number,
    Open,
    Close,
    Comma,
    Colon,
    Dot,
    Pipe,
    Assign,
    /// `+ - * **`: binary or unary by where it stands.
    Sign,
    /// Every other operator.
    Operator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece<'a> {
    pub kind: Kind,
    /// The piece's text in the source.
    pub text: &'a str,
    /// Its byte offset in the source.
    pub start: usize,
    /// A tag's opening and closing delimiters (`{{-`, `%}`), as written.
    pub open: &'a str,
    pub close: &'a str,
    /// A tag's tokens, between its delimiters.
    pub tokens: Vec<Tok<'a>>,
}

impl<'a> Piece<'a> {
    fn plain(kind: Kind, source: &'a str, start: usize, end: usize) -> Piece<'a> {
        Piece {
            kind,
            text: &source[start..end],
            start,
            open: "",
            close: "",
            tokens: Vec::new(),
        }
    }

    /// The first word of a statement: `if`, `endfor`, `set`.
    pub fn keyword(&self) -> &'a str {
        self.tokens
            .first()
            .filter(|t| t.kind == TokKind::Ident)
            .map_or("", |t| t.text)
    }

    pub fn is_jinja(&self) -> bool {
        self.kind != Kind::Sql
    }
}

/// The pieces of a document, or minijinja's complaint about its syntax.
pub fn split(source: &str) -> Result<Vec<Piece<'_>>, String> {
    let tokens: Vec<(Token<'_>, usize, usize)> = tokenize(source, false, SyntaxConfig::default())
        .map(|t| {
            t.map(|(tok, span)| (tok, span.start_offset as usize, span.end_offset as usize))
                .map_err(|e| e.to_string())
        })
        .collect::<Result<_, _>>()?;
    let mut pieces = Vec::new();
    let mut cursor = 0;
    let mut i = 0;
    while i < tokens.len() {
        let (tok, start, end) = (&tokens[i].0, tokens[i].1, tokens[i].2);
        gap(source, cursor, start, &mut pieces);
        cursor = match tok {
            Token::VariableStart | Token::BlockStart => {
                let kind = if matches!(tok, Token::VariableStart) {
                    Kind::Expr
                } else {
                    Kind::Stmt
                };
                let close_at = tokens[i..]
                    .iter()
                    .position(|(t, ..)| matches!(t, Token::VariableEnd | Token::BlockEnd))
                    .map(|n| i + n)
                    .ok_or("an unclosed tag")?;
                let (_, close_start, close_end) = tokens[close_at];
                pieces.push(Piece {
                    kind,
                    text: &source[start..close_end],
                    start,
                    open: &source[start..end],
                    close: &source[close_start..close_end],
                    tokens: tokens[i + 1..close_at]
                        .iter()
                        .map(|(t, s, e)| Tok {
                            kind: tok_kind(t),
                            text: &source[*s..*e],
                        })
                        .collect(),
                });
                i = close_at;
                close_end
            }
            _ => {
                pieces.push(Piece::plain(Kind::Sql, source, start, end));
                end
            }
        };
        i += 1;
    }
    gap(source, cursor, source.len(), &mut pieces);
    Ok(merge_raw(pieces, source))
}

/// What lies between two token spans: whitespace, comments, raw tags.
fn gap<'a>(source: &'a str, mut at: usize, end: usize, pieces: &mut Vec<Piece<'a>>) {
    while at < end {
        let rest = &source[at..end];
        let next = [rest.find("{#"), rest.find("{%")]
            .into_iter()
            .flatten()
            .min();
        let Some(offset) = next else {
            pieces.push(Piece::plain(Kind::Sql, source, at, end));
            return;
        };
        if offset > 0 {
            pieces.push(Piece::plain(Kind::Sql, source, at, at + offset));
        }
        let tag_start = at + offset;
        let (kind, closer) = if rest[offset..].starts_with("{#") {
            (Kind::Comment, "#}")
        } else {
            (Kind::Raw, "%}")
        };
        let tag_end = source[tag_start..end]
            .find(closer)
            .map_or(end, |n| tag_start + n + closer.len());
        pieces.push(Piece::plain(kind, source, tag_start, tag_end));
        at = tag_end;
    }
}

/// A `{% raw %}` tag, the text after it and its `{% endraw %}` as one piece.
fn merge_raw<'a>(pieces: Vec<Piece<'a>>, source: &'a str) -> Vec<Piece<'a>> {
    let mut out: Vec<Piece<'a>> = Vec::with_capacity(pieces.len());
    let mut open: Option<usize> = None;
    for piece in pieces {
        match (open, piece.kind) {
            (None, Kind::Raw) => open = Some(piece.start),
            (Some(start), Kind::Raw) if piece.text.contains("endraw") => {
                out.push(Piece::plain(
                    Kind::Raw,
                    source,
                    start,
                    piece.start + piece.text.len(),
                ));
                open = None;
            }
            (Some(_), _) => {}
            (None, _) => out.push(piece),
        }
    }
    if let Some(start) = open {
        out.push(Piece::plain(Kind::Raw, source, start, source.len()));
    }
    out
}

fn tok_kind(token: &Token<'_>) -> TokKind {
    match token {
        Token::Ident(_) => TokKind::Ident,
        Token::Str(_) | Token::String(_) => TokKind::Str,
        Token::Int(_) | Token::Int128(_) | Token::Float(_) => TokKind::Number,
        Token::ParenOpen | Token::BracketOpen | Token::BraceOpen => TokKind::Open,
        Token::ParenClose | Token::BracketClose | Token::BraceClose => TokKind::Close,
        Token::Comma => TokKind::Comma,
        Token::Colon => TokKind::Colon,
        Token::Dot => TokKind::Dot,
        Token::Pipe => TokKind::Pipe,
        Token::Assign => TokKind::Assign,
        Token::Plus | Token::Minus | Token::Mul | Token::Pow => TokKind::Sign,
        _ => TokKind::Operator,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(source: &str) -> Vec<(Kind, &str)> {
        split(source)
            .unwrap()
            .into_iter()
            .map(|p| (p.kind, p.text))
            .collect()
    }

    #[test]
    fn every_byte_lands_in_one_piece() {
        let source = "{{- config(a=1) }}\nselect {# c #} {%- if x -%} a {% endif %}\
                      {% raw %}{{x}}{% endraw %} {{ a|b(1,k=2) }}\n";
        let joined: String = split(source).unwrap().iter().map(|p| p.text).collect();
        assert_eq!(joined, source);
    }

    #[test]
    fn tags_comments_and_raw_regions() {
        assert_eq!(
            kinds("select {{ x }} {# note #}\n{% raw %}{{ y }}{% endraw %}{% if a %}"),
            vec![
                (Kind::Sql, "select "),
                (Kind::Expr, "{{ x }}"),
                (Kind::Sql, " "),
                (Kind::Comment, "{# note #}"),
                (Kind::Sql, "\n"),
                (Kind::Raw, "{% raw %}{{ y }}{% endraw %}"),
                (Kind::Stmt, "{% if a %}"),
            ]
        );
    }

    #[test]
    fn a_tag_keeps_its_delimiters_and_tokens() {
        let pieces = split("{%- set x = ref('a') -%}").unwrap();
        let tag = &pieces[0];
        assert_eq!((tag.open, tag.close, tag.keyword()), ("{%-", "-%}", "set"));
        let texts: Vec<&str> = tag.tokens.iter().map(|t| t.text).collect();
        assert_eq!(texts, ["set", "x", "=", "ref", "(", "'a'", ")"]);
    }

    #[test]
    fn broken_jinja_is_minijinjas_error() {
        assert!(split("select {{ x ").is_err());
    }
}
