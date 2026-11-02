//! Feature algorithms (03 §9 + FEATURE_REQUESTS): pure functions over the
//! context snapshot and a parsed document — the dispatcher serializes
//! (functional core / imperative shell, 05 §3).

mod completion;
mod definition;
mod diagnostics;
mod hover;
mod navigation;
mod rename;
mod signature;
mod structure;

pub use completion::{completion, resolve_completion_documentation};
pub use definition::{definition, go_to_schema};
pub use diagnostics::dangling_ref_diagnostics;
pub use hover::hover;
pub use navigation::{
    code_lenses, document_highlights, document_symbols, model_graph, references, workspace_symbols,
};
pub use rename::rename_model;
pub use signature::signature_help;
pub use structure::{folding_ranges, selection_ranges, semantic_tokens};

use std::path::Path;

use cogitator_lsp::{Position, Range};
use cogitator_parser::Token;

pub(crate) use crate::context::DbtContext;
pub(crate) use crate::Document;

fn file_uri(path: &Path) -> String {
    cogitator_lsp::fs_path_to_uri(path)
}

pub fn model_name_from_uri(uri: &str) -> Option<String> {
    let path = cogitator_lsp::uri_to_fs_path(uri).unwrap_or_else(|| uri.to_string());
    let base = Path::new(&path).file_name()?.to_string_lossy();
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
