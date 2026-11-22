//! Syntax highlighting from the grammars' own queries, for `dbt.highlight`
//! (specs/highlighting/00-highlighting.md):
//!
//! - `dbt`: tree-sitter-dbt's `highlights.scm` colours the Jinja (layer
//!   `jinja`), and its `injections.scm` hands the SQL between the tags (one
//!   combined document) to tree-sitter-sequel's `highlights.scm` (layer
//!   `sql`);
//! - `sql`: tree-sitter-sequel's alone (dbt's compiled output);
//! - `yaml`: tree-sitter-yaml's (layer `yaml`), with the Jinja and the SQL
//!   inside values and the dbt-aware captures composed on top by the
//!   server (`yaml.rs`), since the YAML grammar has no injections.
//!
//! Pure: text in, spans with their capture names and layers out, so any
//! editor paints them with its own faces (Neovim's `@keyword`,
//! `@tag.delimiter`, … groups).

mod yaml;

use std::sync::OnceLock;

use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

use crate::Point;

pub use yaml::{Reference, YamlContext, YamlFile};

/// Which grammar reads the text: a dbt file (Jinja over SQL), plain SQL
/// such as dbt's compiled output, or a YAML file of the project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HighlightLanguage {
    Dbt,
    Sql,
    Yaml,
}

impl HighlightLanguage {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "dbt" | "jinja" => Some(Self::Dbt),
            "sql" => Some(Self::Sql),
            "yaml" | "yml" => Some(Self::Yaml),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Dbt => "dbt",
            Self::Sql => "sql",
            Self::Yaml => "yaml",
        }
    }

    /// The language of a file, by its path (spec §2): `.yml`/`.yaml` is
    /// YAML, a `.sql` under `target/compiled` or `target/run` is plain SQL,
    /// any other file dbt.
    pub fn of_path(path: &str) -> Self {
        let path = path.replace('\\', "/");
        if path.ends_with(".yml") || path.ends_with(".yaml") {
            Self::Yaml
        } else if path.contains("/target/compiled/")
            || path.contains("/target/run/")
            || path.starts_with("target/compiled/")
            || path.starts_with("target/run/")
        {
            Self::Sql
        } else {
            Self::Dbt
        }
    }
}

/// Which grammar (or the server's own dbt knowledge) produced a span; the
/// order is the precedence of §3.3: a later layer paints over an earlier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Layer {
    Yaml,
    Sql,
    Jinja,
    Dbt,
}

impl Layer {
    pub fn name(self) -> &'static str {
        match self {
            Self::Yaml => "yaml",
            Self::Sql => "sql",
            Self::Jinja => "jinja",
            Self::Dbt => "dbt",
        }
    }
}

/// One highlighted stretch of text, on one line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightSpan {
    pub start: Point,
    pub end: Point,
    /// The capture name, e.g. `keyword.conditional` or `type.dbt.model`.
    pub capture: String,
    pub layer: Layer,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Highlights {
    pub spans: Vec<HighlightSpan>,
    /// More spans than asked for; the rest are left out.
    pub truncated: bool,
    /// The layers that ran, in precedence order.
    pub layers: Vec<Layer>,
}

impl Highlights {
    /// The spans that touch lines `start..end` (end exclusive).
    pub fn in_lines(&self, start: u32, end: u32) -> Vec<HighlightSpan> {
        self.spans
            .iter()
            .filter(|s| s.end.line >= start && s.start.line < end)
            .cloned()
            .collect()
    }
}

/// A span in bytes, before it becomes a range of lines and UTF-16 columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ByteSpan {
    pub start: usize,
    pub end: usize,
    pub capture: String,
    pub layer: Layer,
}

struct Configs {
    dbt: HighlightConfiguration,
    sql: HighlightConfiguration,
    yaml: HighlightConfiguration,
    /// The union of the three queries' capture names.
    names: Vec<String>,
}

/// A highlights query without its spell-check captures: tree-sitter-highlight
/// lets the last capture on a node win, and `(comment) @comment @spell`
/// would leave a comment with `spell`, which is no colour.
fn without_spell(query: &str) -> String {
    query.replace(" @nospell", "").replace(" @spell", "")
}

/// The configurations, built once. Each is configured with the union of
/// the capture names behind a run of placeholders that match no capture:
/// dbt's indices are `0..n`, sql's `n..2n`, yaml's `2n..3n`, so an event's
/// index says which grammar — which layer — produced it, also inside an
/// injection.
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
            let mut yaml = HighlightConfiguration::new(
                tree_sitter_yaml::LANGUAGE.into(),
                "yaml",
                &without_spell(tree_sitter_yaml::HIGHLIGHTS_QUERY),
                "",
                "",
            )
            .ok()?;
            let mut names: Vec<String> = dbt
                .names()
                .iter()
                .chain(sql.names())
                .chain(yaml.names())
                .map(|n| n.to_string())
                .collect();
            names.sort();
            names.dedup();
            // captures that only steer the query, never a colour
            names.retain(|n| !n.starts_with('_') && n != "spell" && n != "nospell");
            let padded = |times: usize| -> Vec<String> {
                let mut out: Vec<String> =
                    (0..names.len() * times).map(|i| format!("~{i}")).collect();
                out.extend(names.iter().cloned());
                out
            };
            dbt.configure(&names);
            sql.configure(&padded(1));
            yaml.configure(&padded(2));
            Some(Configs {
                dbt,
                sql,
                yaml,
                names,
            })
        })
        .as_ref()
}

/// The capture names `highlight` can return, sorted: the three queries'
/// and the server's dbt-aware ones (§5).
pub fn capture_names() -> Vec<String> {
    let mut out = configs().map(|c| c.names.clone()).unwrap_or_default();
    out.extend(yaml::DBT_CAPTURES.iter().map(|c| c.to_string()));
    out.extend(yaml::DBT_CAPTURES.iter().map(|c| format!("{c}.unresolved")));
    out.sort();
    out.dedup();
    out
}

/// The spans one grammar gives `text` (with dbt's injection of SQL), in
/// bytes, innermost capture on top, not overlapping.
pub(crate) fn grammar_spans(text: &str, language: HighlightLanguage) -> Option<Vec<ByteSpan>> {
    let configs = configs()?;
    let config = match language {
        HighlightLanguage::Dbt => &configs.dbt,
        HighlightLanguage::Sql => &configs.sql,
        HighlightLanguage::Yaml => &configs.yaml,
    };
    let n = configs.names.len();
    let mut highlighter = Highlighter::new();
    let events = highlighter
        .highlight(config, text.as_bytes(), None, None, |name| {
            (name == "sql").then_some(&configs.sql)
        })
        .ok()?;
    let mut out: Vec<ByteSpan> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    for event in events {
        match event.ok()? {
            HighlightEvent::HighlightStart(h) => stack.push(h.0),
            HighlightEvent::HighlightEnd => {
                stack.pop();
            }
            HighlightEvent::Source { start, end } => {
                let Some(&top) = stack.last() else { continue };
                if start == end {
                    continue;
                }
                let layer = match top / n {
                    0 if language == HighlightLanguage::Sql => Layer::Sql,
                    0 => Layer::Jinja,
                    1 => Layer::Sql,
                    _ => Layer::Yaml,
                };
                let capture = &configs.names[top % n];
                if let Some(last) = out.last_mut() {
                    if last.end == start && &last.capture == capture && last.layer == layer {
                        last.end = end;
                        continue;
                    }
                }
                out.push(ByteSpan {
                    start,
                    end,
                    capture: capture.clone(),
                    layer,
                });
            }
        }
    }
    Some(out)
}

/// Lay `spans` over `len` bytes by precedence (§3.3): a span of a later
/// layer paints over the stretch it covers; of one layer, the later span in
/// the list wins. Returns maximal runs of one capture and layer.
pub(crate) fn merge(spans: Vec<ByteSpan>, len: usize) -> Vec<ByteSpan> {
    if spans.is_empty() {
        return spans;
    }
    let mut order: Vec<usize> = (0..spans.len()).collect();
    order.sort_by_key(|&i| (spans[i].layer, i));
    let mut owner: Vec<u32> = vec![u32::MAX; len];
    for i in order {
        let s = &spans[i];
        for slot in &mut owner[s.start.min(len)..s.end.min(len)] {
            *slot = i as u32;
        }
    }
    let mut out: Vec<ByteSpan> = Vec::new();
    let mut at = 0;
    while at < len {
        let who = owner[at];
        let mut end = at + 1;
        while end < len && owner[end] == who {
            end += 1;
        }
        if who != u32::MAX {
            let s = &spans[who as usize];
            match out.last_mut() {
                Some(last)
                    if last.end == at && last.capture == s.capture && last.layer == s.layer =>
                {
                    last.end = end;
                }
                _ => out.push(ByteSpan {
                    start: at,
                    end,
                    capture: s.capture.clone(),
                    layer: s.layer,
                }),
            }
        }
        at = end;
    }
    out
}

/// Byte offset → LSP position, from the line starts of the text.
pub(crate) struct Lines<'a> {
    text: &'a str,
    starts: Vec<usize>,
}

impl<'a> Lines<'a> {
    pub(crate) fn new(text: &'a str) -> Self {
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        Self { text, starts }
    }

    pub(crate) fn point(&self, byte: usize) -> Point {
        let line = self.starts.partition_point(|&s| s <= byte) - 1;
        let start = self.starts[line];
        let prefix = self.text.get(start..byte).unwrap_or("");
        Point {
            line: line as u32,
            character: prefix.encode_utf16().count() as u32,
        }
    }
}

/// Byte spans as line-bound spans: split at line breaks, whitespace-only
/// pieces dropped, at most `max_spans`.
fn finish(text: &str, spans: Vec<ByteSpan>, max_spans: usize, layers: Vec<Layer>) -> Highlights {
    let lines = Lines::new(text);
    let mut out = Highlights {
        layers,
        ..Highlights::default()
    };
    'spans: for span in spans {
        let mut start = span.start;
        while start < span.end {
            let end = text[start..span.end]
                .find('\n')
                .map(|i| start + i)
                .unwrap_or(span.end);
            let piece = &text[start..end];
            let trimmed = piece.trim();
            if !trimmed.is_empty() {
                if out.spans.len() == max_spans {
                    out.truncated = true;
                    break 'spans;
                }
                // a piece without the blanks around it
                let lead = piece.len() - piece.trim_start().len();
                let (from, to) = (start + lead, start + lead + trimmed.len());
                out.spans.push(HighlightSpan {
                    start: lines.point(from),
                    end: lines.point(to),
                    capture: span.capture.clone(),
                    layer: span.layer,
                });
            }
            start = end + 1;
        }
    }
    out
}

/// Highlight `text` as `language`, at most `max_spans` spans. YAML gets
/// its base layer, the Jinja and SQL inside it and the dbt-aware captures
/// of a property file with every reference taken as resolved; the server
/// uses [`highlight_with`] to pass what the project knows. `None` when a
/// grammar cannot be loaded or tree-sitter gives up on the text.
pub fn highlight(text: &str, language: HighlightLanguage, max_spans: usize) -> Option<Highlights> {
    highlight_with(text, language, max_spans, &YamlContext::default())
}

/// [`highlight`], with what the project knows for YAML (§5): which file it
/// is, the dbt config keys, and whether a reference resolves.
pub fn highlight_with(
    text: &str,
    language: HighlightLanguage,
    max_spans: usize,
    context: &YamlContext,
) -> Option<Highlights> {
    match language {
        HighlightLanguage::Dbt => {
            let spans = grammar_spans(text, language)?;
            Some(finish(
                text,
                spans,
                max_spans,
                vec![Layer::Sql, Layer::Jinja],
            ))
        }
        HighlightLanguage::Sql => {
            let spans = grammar_spans(text, language)?;
            Some(finish(text, spans, max_spans, vec![Layer::Sql]))
        }
        HighlightLanguage::Yaml => {
            let spans = yaml::spans(text, context)?;
            let merged = merge(spans, text.len());
            Some(finish(
                text,
                merged,
                max_spans,
                vec![Layer::Yaml, Layer::Sql, Layer::Jinja, Layer::Dbt],
            ))
        }
    }
}

#[cfg(test)]
mod tests;
