//! Project analysis: state, dbt context, features (specs/rust-port/03).

pub mod context;
pub mod features;
pub mod graph;
pub mod util;

use std::collections::HashMap;
use std::sync::Arc;

use dbt_ls_dialects::Dialect;
use dbt_ls_lsp::ContentChange;
use dbt_ls_parser::{DocumentParser, JinjaSqlParser, ParsedDocument};

pub use context::{
    build_context, DbtContext, MacroDetails, ModelDetails, SourceDetails, VariableDetails,
};

/// One open document: the current text plus the last good parse (05 §4
/// step 1 — on tokenize failure `parsed` goes stale while `text` advances).
#[derive(Debug, Default)]
pub struct Document {
    pub text: Arc<str>,
    pub parsed: ParsedDocument,
}

/// Open-document store owned by the dispatcher (05 §6.2 — single owner, no
/// lock).
#[derive(Default)]
pub struct Documents {
    parser: JinjaSqlParser,
    docs: HashMap<String, Document>,
}

impl Documents {
    pub fn get(&self, uri: &str) -> Option<&Document> {
        self.docs.get(uri)
    }

    pub fn open(&mut self, uri: String, text: &str, dialect: &Dialect) {
        let parsed = self.parse_or_last_good(None, text, dialect);
        self.docs.insert(
            uri,
            Document {
                text: Arc::from(text),
                parsed,
            },
        );
    }

    /// didChange (03 §2): fold the changes, reparse, keep last good tokens on
    /// malformed input. Unknown uri is a no-op like Go.
    pub fn change(&mut self, uri: &str, changes: &[ContentChange<'_>], dialect: &Dialect) {
        let Some(doc) = self.docs.get(uri) else {
            return;
        };
        let mut text = doc.text.to_string();
        for change in changes {
            if change.range.is_zero() {
                text = change.text.to_string();
            } else {
                text = util::apply_incremental_change(&text, change.range, &change.text);
            }
        }
        let previous = self.docs.remove(uri).map(|d| d.parsed);
        let parsed = self.parse_or_last_good(previous, &text, dialect);
        self.docs.insert(
            uri.to_string(),
            Document {
                text: Arc::from(text.as_str()),
                parsed,
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
            self.docs.insert(
                uri,
                Document {
                    text: Arc::from(text.as_str()),
                    parsed,
                },
            );
        }
    }
}
