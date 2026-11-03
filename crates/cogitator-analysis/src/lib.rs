//! Project analysis: state, dbt context, features (specs/rust-port/03).

pub mod artifacts;
pub mod cache;
pub mod config_schema;
pub mod context;
pub mod features;
pub mod graph;
pub mod lineage;
pub mod profiles;
pub mod util;
pub mod yaml_cogitator;
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
    docs: HashMap<String, Arc<Document>>,
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
        self.docs.get(uri).map(|doc| doc.as_ref())
    }

    /// A shareable handle for offloaded work: the document outlives any
    /// later didChange (which replaces the Arc, never mutates it).
    pub fn get_arc(&self, uri: &str) -> Option<Arc<Document>> {
        self.docs.get(uri).cloned()
    }

    /// textDocument/didClose (P1): forget the document (and whatever
    /// per-document state the parser backend keeps).
    pub fn close(&mut self, uri: &str) {
        self.docs.remove(uri);
        self.parser.forget(uri);
    }

    pub fn open(&mut self, uri: String, text: &str, dialect: &Dialect) {
        let parsed = self.parse_or_last_good(&uri, None, text, dialect, &[]);
        self.generations += 1;
        self.docs.insert(
            uri,
            Arc::new(Document {
                text: Arc::from(text),
                parsed,
                generation: self.generations,
            }),
        );
    }

    /// didChange (03 §2): fold the changes, reparse, keep last good tokens on
    /// malformed input. Unknown uri is a no-op like Go.
    pub fn change(&mut self, uri: &str, changes: &[ContentChange<'_>], dialect: &Dialect) {
        let Some(doc) = self.docs.get(uri) else {
            return;
        };
        // fold the edits, collecting tree-sitter-shaped records while
        // every change stays ranged (a full-text change resets them)
        let mut edits: Vec<cogitator_parser::AppliedEdit> = Vec::new();
        let text = changes.iter().fold(doc.text.to_string(), |text, change| {
            if change.range.is_zero() {
                edits.clear();
                change.text.to_string()
            } else {
                match util::edit_for_change(&text, change.range, &change.text) {
                    Some(edit) => edits.push(edit),
                    None => edits.clear(),
                }
                util::apply_incremental_change(&text, change.range, &change.text)
            }
        });
        // last-good reuse is move-out when nobody else holds the Arc; an
        // offloaded task still reading the old snapshot forces one deep
        // clone (the owner boundary that clone feeds, 08 §1.5)
        let previous = self.docs.remove(uri).map(|d| match Arc::try_unwrap(d) {
            Ok(owned) => owned.parsed,
            Err(shared) => shared.parsed.clone(),
        });
        let parsed = self.parse_or_last_good(uri, previous, &text, dialect, &edits);
        self.generations += 1;
        self.docs.insert(
            uri.to_string(),
            Arc::new(Document {
                text: Arc::from(text.as_str()),
                parsed,
                generation: self.generations,
            }),
        );
    }

    fn parse_or_last_good(
        &self,
        uri: &str,
        previous: Option<ParsedDocument>,
        text: &str,
        dialect: &Dialect,
        edits: &[cogitator_parser::AppliedEdit],
    ) -> ParsedDocument {
        match self.parser.parse_incremental(uri, text, dialect, edits) {
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
            let previous = self.docs.remove(&uri).map(|d| match Arc::try_unwrap(d) {
                Ok(owned) => owned.parsed,
                Err(shared) => shared.parsed.clone(),
            });
            let parsed = self.parse_or_last_good(&uri, previous, &text, dialect, &[]);
            self.generations += 1;
            self.docs.insert(
                uri,
                Arc::new(Document {
                    text: Arc::from(text.as_str()),
                    parsed,
                    generation: self.generations,
                }),
            );
        }
    }
}
