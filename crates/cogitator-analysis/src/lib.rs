//! Project analysis: state, dbt context, features (specs/rust-port/03).

pub mod cache;
pub mod context;
pub mod features;
pub mod graph;
pub mod lineage;
pub mod profiles;
pub mod util;
pub mod yaml_features;

use std::collections::HashMap;
use std::sync::Arc;

use cogitator_dialects::Dialect;
use cogitator_lsp::ContentChange;
use cogitator_parser::{DocumentParser, JinjaSqlParser, ParsedDocument};

pub use context::{
    build_context, ContextBuilder, DbtContext, MacroDetails, ModelDetails, SourceDetails,
    VariableDetails,
};

/// One open document: the current text plus the last good parse (05 §4
/// step 1 — on tokenize failure `parsed` goes stale while `text` advances).
#[derive(Debug, Default)]
pub struct Document {
    pub text: Arc<str>,
    pub parsed: ParsedDocument,
    /// Monotonic per-store counter value at the last open/change: the
    /// key derived caches use to spot edits without hashing text.
    pub generation: u64,
}

/// Open-document store owned by the dispatcher (05 §6.2 — single owner, no
/// lock). The tokenization backend is injected through the `DocumentParser`
/// seam (06 §3.5): phase-1 `JinjaSqlParser` by default, the tree-sitter
/// backend when the binary selects it.
pub struct Documents {
    parser: Box<dyn DocumentParser + Send>,
    docs: HashMap<String, Document>,
    generations: u64,
}

impl Default for Documents {
    fn default() -> Self {
        Documents::with_parser(Box::new(JinjaSqlParser))
    }
}

impl Documents {
    pub fn with_parser(parser: Box<dyn DocumentParser + Send>) -> Self {
        Documents {
            parser,
            docs: HashMap::new(),
            generations: 0,
        }
    }

    pub fn get(&self, uri: &str) -> Option<&Document> {
        self.docs.get(uri)
    }

    /// textDocument/didClose (P1): forget the document.
    pub fn close(&mut self, uri: &str) {
        self.docs.remove(uri);
    }

    pub fn open(&mut self, uri: String, text: &str, dialect: &Dialect) {
        let parsed = self.parse_or_last_good(None, text, dialect);
        self.generations += 1;
        self.docs.insert(
            uri,
            Document {
                text: Arc::from(text),
                parsed,
                generation: self.generations,
            },
        );
    }

    /// didChange (03 §2): fold the changes, reparse, keep last good tokens on
    /// malformed input. Unknown uri is a no-op like Go.
    pub fn change(&mut self, uri: &str, changes: &[ContentChange<'_>], dialect: &Dialect) {
        let Some(doc) = self.docs.get(uri) else {
            return;
        };
        let text = changes.iter().fold(doc.text.to_string(), |text, change| {
            if change.range.is_zero() {
                change.text.to_string()
            } else {
                util::apply_incremental_change(&text, change.range, &change.text)
            }
        });
        let previous = self.docs.remove(uri).map(|d| d.parsed);
        let parsed = self.parse_or_last_good(previous, &text, dialect);
        self.generations += 1;
        self.docs.insert(
            uri.to_string(),
            Document {
                text: Arc::from(text.as_str()),
                parsed,
                generation: self.generations,
            },
        );
    }

    fn parse_or_last_good(
        &self,
        previous: Option<ParsedDocument>,
        text: &str,
        dialect: &Dialect,
    ) -> ParsedDocument {
        match self.parser.parse(text, dialect) {
            Ok(parsed) => parsed,
            Err(err) => {
                tracing::trace!(%err, "parse failed; serving last good document");
                previous.unwrap_or_default()
            }
        }
    }

    /// Re-parse every open document (dialect may have changed on refresh).
    pub fn reparse_all(&mut self, dialect: &Dialect) {
        let uris: Vec<String> = self.docs.keys().cloned().collect();
        for uri in uris {
            let Some(doc) = self.docs.get(&uri) else {
                continue;
            };
            let text = doc.text.to_string();
            let previous = self.docs.remove(&uri).map(|d| d.parsed);
            let parsed = self.parse_or_last_good(previous, &text, dialect);
            self.generations += 1;
            self.docs.insert(
                uri,
                Document {
                    text: Arc::from(text.as_str()),
                    parsed,
                    generation: self.generations,
                },
            );
        }
    }
}
