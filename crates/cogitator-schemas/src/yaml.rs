//! Marked-YAML helpers (specs/rust-port/08 §2.4): `dbt-schemas`' own, with
//! cogitator's error and LSP positions.
//!
//! Each dbt YAML file is parsed exactly once with saphyr; the marked tree
//! yields (a) a `serde_json::Value` fed into the typify-generated types and
//! (b) node positions for the fields features need ranges for.

use std::path::Path;

pub use dbt_schemas::yaml::{as_str, entries, get, items, to_json, MarkedYaml};

use cogitator_error::{Error, Result};
use cogitator_lsp::{Position, Range};

/// Parse the first YAML document of `text`.
pub fn load_marked<'a>(text: &'a str, path: &Path) -> Result<MarkedYaml<'a>> {
    dbt_schemas::yaml::load_marked(text, path).map_err(|e| match e {
        dbt_schemas::Error::Yaml { path, message } => Error::Yaml { path, message },
        dbt_schemas::Error::Shape { source, .. } => Error::Json(source),
        other => Error::Yaml {
            path: path.to_path_buf(),
            message: other.to_string(),
        },
    })
}

fn lsp(p: dbt_schemas::Position) -> Position {
    Position {
        line: p.line,
        character: p.character,
    }
}

/// 0-based LSP position of a node's start (saphyr lines are 1-based,
/// columns 0-based — mirrors Go's `line-1` / `column-1` on go-yaml's 1-based
/// columns).
pub fn position_of(node: &MarkedYaml<'_>) -> Position {
    lsp(dbt_schemas::yaml::position_of(node))
}

/// 0-based LSP range of a node (start to end, end exclusive).
pub fn range_of(node: &MarkedYaml<'_>) -> Range {
    let span = dbt_schemas::yaml::span_of(node);
    Range {
        start: lsp(span.start),
        end: lsp(span.end),
    }
}

/// YAML syntax-error probe for diagnostics: 0-based position + message.
pub fn syntax_error(text: &str) -> Option<(u32, u32, String)> {
    dbt_schemas::yaml::syntax_error(text).map(|(at, message)| (at.line, at.character, message))
}
