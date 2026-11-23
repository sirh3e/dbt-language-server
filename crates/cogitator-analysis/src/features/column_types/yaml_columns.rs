//! Where a properties file writes each model column, and its `data_type:`.
//! Read with saphyr's marked tree, so positions are the parser's own.

use cogitator_lsp::{Position, Range};
use cogitator_schemas::yaml::{as_str, get, items, position_of, range_of};
use saphyr::{LoadableYamlNode, MarkedYaml};

/// One `columns:` item of a model entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WrittenColumn {
    pub model: String,
    pub name: String,
    pub name_range: Range,
    /// The `data_type:` value as written, with its range.
    pub data_type: Option<(String, Range)>,
    /// Where the column's keys start (the column after `- `).
    pub key_start: Position,
    /// Written in flow style (`- {name: x}`): line edits would break it.
    pub flow: bool,
}

/// A model entry: its name, the lines it spans and its columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WrittenModel {
    pub name: String,
    pub first_line: u32,
    pub last_line: u32,
    pub columns: Vec<WrittenColumn>,
}

/// Every `models:` entry of `text`; empty when it does not parse.
pub(crate) fn written_models(text: &str) -> Vec<WrittenModel> {
    let Ok(docs) = MarkedYaml::load_from_str(text) else {
        return Vec::new();
    };
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    for doc in &docs {
        let Some(models) = get(doc, "models") else {
            continue;
        };
        for entry in items(models) {
            let Some(name) = get(entry, "name").and_then(as_str) else {
                continue;
            };
            let span = range_of(entry);
            let columns = get(entry, "columns")
                .map(|cols| {
                    items(cols)
                        .filter_map(|c| written_column(name, c, &lines))
                        .collect()
                })
                .unwrap_or_default();
            out.push(WrittenModel {
                name: name.to_string(),
                first_line: span.start.line,
                last_line: span.end.line,
                columns,
            });
        }
    }
    out
}

fn written_column(model: &str, node: &MarkedYaml<'_>, lines: &[&str]) -> Option<WrittenColumn> {
    let name_node = get(node, "name")?;
    let name = as_str(name_node)?;
    let data_type =
        get(node, "data_type").and_then(|v| Some((as_str(v)?.to_string(), value_range(v, lines))));
    let key_start = position_of(node);
    let flow = lines
        .get(key_start.line as usize)
        .is_some_and(|l| l.contains('{'));
    Some(WrittenColumn {
        model: model.to_string(),
        name: name.to_string(),
        name_range: value_range(name_node, lines),
        data_type,
        key_start,
        flow,
    })
}

/// A scalar's range on its line: saphyr's start, to the end of the
/// trimmed value (a plain scalar's span ends where the next token starts).
pub(crate) fn value_range(node: &MarkedYaml<'_>, lines: &[&str]) -> Range {
    let start = position_of(node);
    let line = lines.get(start.line as usize).copied().unwrap_or("");
    let text = as_str(node).unwrap_or("");
    let rest = line.get(start.character as usize..).unwrap_or("");
    let quoted = usize::from(rest.starts_with(['"', '\'']));
    let len = if rest[quoted..].starts_with(text) {
        text.encode_utf16().count() + 2 * quoted
    } else {
        rest.trim_end().encode_utf16().count()
    };
    Range {
        start,
        end: Position {
            line: start.line,
            character: start.character + len as u32,
        },
    }
}
