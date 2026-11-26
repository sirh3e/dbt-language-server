//! Jinja out of the parser's way, and back afterwards.
//!
//! sqlfluff parses SQL, not templates, so every tag is replaced by a token
//! the SQL grammar takes where the tag stood, and put back once the layout
//! is printed:
//!
//! - a `{{ … }}` becomes an identifier as wide as the tag, which a column,
//!   a table or a part of a name all take;
//! - any other tag, and a `{{ … }}` on its own line before the model's SQL
//!   (`config()`, `set` calls), becomes a block comment, which goes anywhere.
//!
//! Which is which depends only on the source, never on how it was laid
//! out, so formatting a formatted model masks it the same way.
//!
//! Placeholders start with a prefix the source does not contain, and each
//! must come back exactly once, or the document is declined.

use std::collections::HashMap;

use crate::jinja::{self, Kind, Piece};
use crate::sql::Placeholders;
#[cfg(test)]
use crate::sql::TagRole;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The whole document as one SQL text; `header_as_tags`: a `{{ … }}`
    /// on its own line before any SQL becomes a comment, not an identifier.
    Whole { header_as_tags: bool },
    /// The SQL between tags that stand on their own lines, each parsed on
    /// its own: for documents whose Jinja branches are whole statements.
    Segments,
}

/// What the parser is given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unit {
    Sql(String),
    /// A tag on its own line, after `newlines` line breaks.
    Tag { placeholder: String, newlines: usize },
}

#[derive(Debug, Clone)]
pub struct Masked {
    pub units: Vec<Unit>,
    pub tags: Placeholders,
    /// Each placeholder and the tag that comes back in its place.
    pub restore: Vec<(String, String)>,
}

/// A prefix for placeholders that nothing in the source contains.
pub fn prefix(source: &str) -> String {
    let lower = source.to_lowercase();
    (0..)
        .map(|n| format!("cgfmt{n}x"))
        .find(|p| !lower.contains(p.as_str()))
        .expect("an unused prefix")
}

/// Whether a piece is alone on its line(s).
pub fn alone(source: &str, piece: &Piece<'_>) -> bool {
    let before = &source[..piece.start];
    let line_start = before.rfind('\n').map_or(0, |n| n + 1);
    let end = piece.start + piece.text.len();
    let line_end = source[end..].find('\n').map_or(source.len(), |n| end + n);
    before[line_start..].trim().is_empty() && source[end..line_end].trim().is_empty()
}

/// The parser's input for one way of masking.
pub fn mask(source: &str, pieces: &[Piece<'_>], written: &[String], mode: Mode) -> Masked {
    let prefix = prefix(source);
    let mut units: Vec<Unit> = vec![Unit::Sql(String::new())];
    let mut roles = HashMap::new();
    let mut restore = Vec::new();
    // no SQL so far: what stands here is the model's header
    let mut in_header = true;
    for (i, (piece, text)) in pieces.iter().zip(written).enumerate() {
        let lone = alone(source, piece);
        let header = lone && in_header;
        in_header &= piece.kind != Kind::Sql || piece.text.trim().is_empty();
        let as_ident = piece.kind == Kind::Expr
            && !(header && mode != Mode::Whole { header_as_tags: false });
        let placeholder = match piece.kind {
            Kind::Sql => {
                push_sql(&mut units, piece.text);
                continue;
            }
            _ if as_ident => identifier(&prefix, i, text),
            _ => comment(&prefix, i, if lone { 0 } else { width(text) }),
        };
        roles.insert(placeholder.clone(), jinja::role(piece));
        restore.push((placeholder.clone(), text.clone()));
        if mode == Mode::Segments && lone && !as_ident {
            let newlines = trailing_newlines(&units);
            units.push(Unit::Tag {
                placeholder,
                newlines,
            });
            units.push(Unit::Sql(String::new()));
        } else {
            push_sql(&mut units, &placeholder);
        }
    }
    let units = units
        .into_iter()
        .filter(|u| !matches!(u, Unit::Sql(s) if s.trim().is_empty()))
        .collect();
    Masked {
        units,
        tags: Placeholders { prefix, roles },
        restore,
    }
}

fn push_sql(units: &mut [Unit], text: &str) {
    if let Some(Unit::Sql(sql)) = units.last_mut() {
        sql.push_str(text);
    }
}

/// Line breaks after the last text of the SQL so far: before a lone tag.
fn trailing_newlines(units: &[Unit]) -> usize {
    match units.last() {
        Some(Unit::Sql(sql)) if !sql.trim().is_empty() => {
            let tail = &sql[sql.trim_end().len()..];
            tail.matches('\n').count()
        }
        // after another lone tag: its own line ends, then the blank ones
        Some(Unit::Sql(sql)) => sql.matches('\n').count(),
        _ => 0,
    }
}

fn width(text: &str) -> usize {
    text.lines().next().unwrap_or("").chars().count()
}

/// `x` ends the number, so no placeholder is the start of another
/// (`…e1x` is no part of `…e10x`).
fn identifier(prefix: &str, i: usize, text: &str) -> String {
    let name = format!("{prefix}e{i}x");
    let pad = width(text).saturating_sub(name.len());
    format!("{name}{}", "_".repeat(pad))
}

fn comment(prefix: &str, i: usize, width: usize) -> String {
    let name = format!("/*{prefix}s{i}x");
    let pad = width.saturating_sub(name.len() + 2);
    format!("{name}{}*/", " ".repeat(pad))
}

/// The printed text with every tag back, or None when a placeholder is
/// missing or doubled.
pub fn unmask(printed: &str, restore: &[(String, String)]) -> Option<String> {
    restore
        .iter()
        .try_fold(printed.to_string(), |out, (placeholder, tag)| {
            (out.matches(placeholder.as_str()).count() == 1)
                .then(|| out.replacen(placeholder.as_str(), tag, 1))
        })
}

/// A message with every placeholder in it back as its tag.
pub fn unmask_all(text: &str, restore: &[(String, String)]) -> String {
    restore
        .iter()
        .fold(text.to_string(), |out, (placeholder, tag)| {
            out.replace(placeholder.as_str(), tag)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn masked(source: &str, mode: Mode) -> Masked {
        let pieces = jinja::split(source).unwrap();
        let written: Vec<String> = pieces.iter().map(|p| p.text.to_string()).collect();
        mask(source, &pieces, &written, mode)
    }

    #[test]
    fn inline_expressions_become_identifiers_and_the_rest_comments() {
        let m = masked(
            "{{ config(m='t') }}\nselect {{ col }} from {{ ref('x') }} {% if a %}\n",
            Mode::Whole {
                header_as_tags: true,
            },
        );
        let [Unit::Sql(sql)] = m.units.as_slice() else {
            panic!("{:?}", m.units)
        };
        assert_eq!(
            sql,
            "/*cgfmt0xs0x*/\nselect cgfmt0xe2x from cgfmt0xe4x____ /*cgfmt0xs6x*/\n"
        );
    }

    #[test]
    fn segments_cut_at_lone_tags() {
        let m = masked("select 1\n{% else %}\nselect 2\n", Mode::Segments);
        assert_eq!(
            m.units,
            vec![
                Unit::Sql("select 1\n".into()),
                Unit::Tag {
                    placeholder: "/*cgfmt0xs1x*/".into(),
                    newlines: 1
                },
                Unit::Sql("\nselect 2\n".into()),
            ]
        );
        assert_eq!(m.tags.roles["/*cgfmt0xs1x*/"], TagRole::Mid);
    }

    #[test]
    fn only_the_header_takes_lone_expressions_as_tags() {
        let source = "{{ config(m='t') }}\nselect\n    {{ star() }}\nfrom t\n";
        let m = masked(source, Mode::Whole { header_as_tags: true });
        let [Unit::Sql(sql)] = m.units.as_slice() else {
            panic!("{:?}", m.units)
        };
        assert!(sql.starts_with("/*cgfmt0xs0x*/\nselect\n    cgfmt0xe2x"), "{sql}");
    }

    #[test]
    fn no_placeholder_is_the_start_of_another() {
        let one = identifier("p", 1, "");
        let ten = identifier("p", 10, "");
        assert!(!ten.contains(&one) && !one.contains(&ten));
    }

    #[test]
    fn a_prefix_the_source_has_is_not_taken() {
        assert_eq!(prefix("select cgfmt0x"), "cgfmt1x");
    }

    #[test]
    fn unmask_needs_each_placeholder_once() {
        let restore = vec![("P0".to_string(), "{{ x }}".to_string())];
        assert_eq!(unmask("a P0 b", &restore).as_deref(), Some("a {{ x }} b"));
        assert_eq!(unmask("a b", &restore), None);
        assert_eq!(unmask("P0 P0", &restore), None);
    }
}
