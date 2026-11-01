//! Feature algorithms (03 §9 + FEATURE_REQUESTS): pure functions over the
//! context snapshot and a parsed document — the dispatcher serializes
//! (functional core / imperative shell, 05 §3).

mod completion;
mod definition;
mod diagnostics;
mod hover;
mod navigation;
mod signature;
mod structure;

pub use completion::{completion, resolve_completion_documentation};
pub use definition::{definition, go_to_schema};
pub use diagnostics::dangling_ref_diagnostics;
pub use hover::hover;
pub use navigation::{document_highlights, document_symbols, references, workspace_symbols};
pub use signature::signature_help;
pub use structure::{folding_ranges, selection_ranges, semantic_tokens};

use std::path::Path;

use dbt_ls_lsp::{Position, Range};
use dbt_ls_parser::Token;

pub(crate) use crate::context::DbtContext;
pub(crate) use crate::Document;

fn file_uri(path: &Path) -> String {
    format!("file://{}", path.display())
}

pub fn model_name_from_uri(uri: &str) -> Option<String> {
    let path = uri.strip_prefix("file://").unwrap_or(uri);
    let base = Path::new(path).file_name()?.to_string_lossy();
    base.strip_suffix(".sql").map(str::to_string)
}

// ---------------------------------------------------------------------------
// Completion (03 §9.4)
// ---------------------------------------------------------------------------

pub(crate) fn token_range(t: &Token) -> Range {
    Range {
        start: Position {
            line: t.line,
            character: t.column,
        },
        end: Position {
            line: t.line,
            character: t.column + t.len(),
        },
    }
}

pub(crate) fn display_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}
