//! Syntax highlighting from tree-sitter-dbt's own queries, for
//! `dbt.highlight`: the grammar's `highlights.scm` colours the Jinja, and
//! its `injections.scm` hands the SQL between the tags (one combined
//! document) to tree-sitter-sequel's `highlights.scm`. Pure: text in,
//! spans with their capture names out, so any editor paints them with its
//! own faces (Neovim's `@keyword`, `@tag.delimiter`, … groups).

use std::sync::OnceLock;

use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

use crate::Point;

/// Which grammar reads the text: a dbt file (Jinja over SQL), or plain SQL
/// such as dbt's compiled output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighlightLanguage {
    Dbt,
    Sql,
}

impl HighlightLanguage {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "dbt" | "jinja" => Some(Self::Dbt),
            "sql" => Some(Self::Sql),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Dbt => "dbt",
            Self::Sql => "sql",
        }
    }
}

/// One highlighted stretch of text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightSpan {
    pub start: Point,
    pub end: Point,
    /// The query's capture name, e.g. `keyword.conditional`.
    pub capture: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Highlights {
    pub spans: Vec<HighlightSpan>,
    /// More spans than asked for; the rest are left out.
    pub truncated: bool,
}

struct Configs {
    dbt: HighlightConfiguration,
    sql: HighlightConfiguration,
    names: Vec<String>,
}

/// A highlights query without its spell-check captures: tree-sitter-highlight
/// lets the last capture on a node win, and `(comment) @comment @spell`
/// would leave a comment with `spell`, which is no colour.
fn without_spell(query: &str) -> String {
    query.replace(" @nospell", "").replace(" @spell", "")
}

/// Both configurations, configured with the union of their capture names,
/// built once.
fn configs() -> Option<&'static Configs> {
    static CONFIGS: OnceLock<Option<Configs>> = OnceLock::new();
    CONFIGS
        .get_or_init(|| {
            let mut dbt = HighlightConfiguration::new(
                tree_sitter_dbt::LANGUAGE.into(),
                "dbt",
                &without_spell(tree_sitter_dbt::HIGHLIGHTS_QUERY),
                tree_sitter_dbt::INJECTIONS_QUERY,
                "",
            )
            .ok()?;
            let mut sql = HighlightConfiguration::new(
                tree_sitter_sequel::LANGUAGE.into(),
                "sql",
                &without_spell(tree_sitter_sequel::HIGHLIGHTS_QUERY),
                "",
                "",
            )
            .ok()?;
            let mut names: Vec<String> = dbt
                .names()
                .iter()
                .chain(sql.names())
                .map(|n| n.to_string())
                .collect();
            names.sort();
            names.dedup();
            // captures that only steer the query, never a colour
            names.retain(|n| !n.starts_with('_') && n != "spell" && n != "nospell");
            dbt.configure(&names);
            sql.configure(&names);
            Some(Configs { dbt, sql, names })
        })
        .as_ref()
}

/// The capture names `highlight` can return, sorted.
pub fn capture_names() -> Vec<String> {
    configs().map(|c| c.names.clone()).unwrap_or_default()
}

/// Byte offset → LSP position, from the line starts of the text.
struct Lines<'a> {
    text: &'a str,
    starts: Vec<usize>,
}

impl<'a> Lines<'a> {
    fn new(text: &'a str) -> Self {
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        Self { text, starts }
    }

    fn point(&self, byte: usize) -> Point {
        let line = self.starts.partition_point(|&s| s <= byte) - 1;
        let start = self.starts[line];
        let prefix = self.text.get(start..byte).unwrap_or("");
        Point {
            line: line as u32,
            character: prefix.encode_utf16().count() as u32,
        }
    }
}

/// Highlight `text` as `language`, at most `max_spans` spans. `None` when a
/// grammar cannot be loaded or tree-sitter gives up on the text.
pub fn highlight(text: &str, language: HighlightLanguage, max_spans: usize) -> Option<Highlights> {
    let configs = configs()?;
    let config = match language {
        HighlightLanguage::Dbt => &configs.dbt,
        HighlightLanguage::Sql => &configs.sql,
    };
    let mut highlighter = Highlighter::new();
    let events = highlighter
        .highlight(config, text.as_bytes(), None, None, |name| {
            (name == "sql").then_some(&configs.sql)
        })
        .ok()?;

    let lines = Lines::new(text);
    let mut out = Highlights::default();
    // the innermost capture wins: a stack of the open ones
    let mut stack: Vec<usize> = Vec::new();
    for event in events {
        match event.ok()? {
            HighlightEvent::HighlightStart(h) => stack.push(h.0),
            HighlightEvent::HighlightEnd => {
                stack.pop();
            }
            HighlightEvent::Source { start, end } => {
                let Some(&top) = stack.last() else { continue };
                if start == end || text[start..end].trim().is_empty() {
                    continue;
                }
                let capture = &configs.names[top];
                // one span per stretch: a capture split by injection
                // boundaries joins up again
                if let Some(last) = out.spans.last_mut() {
                    if &last.capture == capture && last.end == lines.point(start) {
                        last.end = lines.point(end);
                        continue;
                    }
                }
                if out.spans.len() == max_spans {
                    out.truncated = true;
                    break;
                }
                out.spans.push(HighlightSpan {
                    start: lines.point(start),
                    end: lines.point(end),
                    capture: capture.clone(),
                });
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn captures(text: &str, language: HighlightLanguage) -> Vec<(String, String)> {
        let h = highlight(text, language, 10_000).expect("grammars load");
        let lines: Vec<&str> = text.split('\n').collect();
        h.spans
            .iter()
            .map(|s| {
                assert_eq!(s.start.line, s.end.line, "{s:?} spans lines");
                let line: Vec<u16> = lines[s.start.line as usize].encode_utf16().collect();
                let piece = String::from_utf16_lossy(
                    &line[s.start.character as usize..s.end.character as usize],
                );
                (piece, s.capture.clone())
            })
            .collect()
    }

    fn capture_of(found: &[(String, String)], piece: &str) -> String {
        found
            .iter()
            .find(|(p, _)| p == piece)
            .unwrap_or_else(|| panic!("no span for {piece:?} in {found:?}"))
            .1
            .clone()
    }

    #[test]
    fn jinja_and_the_sql_between_the_tags_are_both_highlighted() {
        let found = captures(
            "select o.id from {{ ref('orders') }} o\n{% if is_incremental() %}\nwhere o.ts > 1\n{% endif %}",
            HighlightLanguage::Dbt,
        );
        assert_eq!(capture_of(&found, "{{"), "tag.delimiter");
        // dbt's own functions are builtins to the grammar
        assert_eq!(capture_of(&found, "ref"), "function.builtin");
        assert_eq!(capture_of(&found, "'orders'"), "string");
        assert_eq!(capture_of(&found, "if"), "keyword.conditional");
        // the SQL is the injected grammar's
        assert!(capture_of(&found, "select").starts_with("keyword"));
        assert!(capture_of(&found, "where").starts_with("keyword"));
    }

    #[test]
    fn plain_sql_takes_the_sql_grammar_alone() {
        let found = captures(
            "select count(*) from analytics.orders",
            HighlightLanguage::Sql,
        );
        assert!(capture_of(&found, "select").starts_with("keyword"));
        assert!(!found.iter().any(|(_, c)| c == "tag.delimiter"));
    }

    #[test]
    fn columns_count_utf16_code_units() {
        let h = highlight("-- 🦀\nselect 1", HighlightLanguage::Sql, 100).unwrap();
        let select = h
            .spans
            .iter()
            .find(|s| s.start.line == 1 && s.capture.starts_with("keyword"))
            .unwrap();
        assert_eq!((select.start.character, select.end.character), (0, 6));
        let comment = h
            .spans
            .iter()
            .find(|s| s.capture == "comment")
            .expect("the comment");
        // "-- 🦀" is 5 UTF-16 code units: the crab is a surrogate pair
        assert_eq!(comment.end.character, 5);
    }

    #[test]
    fn spans_stop_at_the_cap() {
        let h = highlight("select a, b, c, d from t", HighlightLanguage::Sql, 2).unwrap();
        assert_eq!(h.spans.len(), 2);
        assert!(h.truncated);
    }

    #[test]
    fn the_capture_names_are_known_and_clean() {
        let names = capture_names();
        assert!(names.iter().any(|n| n == "tag.delimiter"));
        assert!(names.iter().any(|n| n.starts_with("keyword")));
        assert!(!names.iter().any(|n| n.starts_with('_') || n == "spell"));
    }

    #[test]
    fn language_names_round_trip() {
        for l in [HighlightLanguage::Dbt, HighlightLanguage::Sql] {
            assert_eq!(HighlightLanguage::parse(l.name()), Some(l));
        }
        assert_eq!(HighlightLanguage::parse("python"), None);
    }
}
