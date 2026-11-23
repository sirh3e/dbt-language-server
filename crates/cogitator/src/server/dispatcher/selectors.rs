//! Selector-engine requests, offloaded: each builds the project's
//! universe from files (`server/selectors.rs`).

use cogitator_analysis::features::selectors_yml;
use cogitator_lsp::HoverResult;

use super::Dispatcher;
use crate::server::reply::Reply;
use crate::server::selectors::{self, Open};

impl Dispatcher {
    /// textDocument/hover on selectors.yml; `false` for any other file.
    pub(super) fn selectors_hover(&mut self, content: &[u8]) -> bool {
        let Some((id, uri, pos)) = Self::position_request(content) else {
            return false;
        };
        if !selectors_yml::is_selectors_file(&uri) {
            return false;
        }
        let snap = self.snapshot_for(&uri);
        let open = Open::of(self.documents.texts());
        self.offload_blocking(id, "textDocument/hover", move || {
            let contents = snap
                .doc
                .as_deref()
                .and_then(|doc| selectors::hover(&snap.ctx, open, &doc.text, pos))
                .unwrap_or_default();
            Reply::result(&HoverResult { contents })
        });
        true
    }

    /// textDocument/completion in a selector or tag slot (selectors.yml,
    /// `tags:` of dbt_project.yml and properties files); `false` when the
    /// cursor is in none.
    pub(super) fn selectors_completion(&mut self, content: &[u8]) -> bool {
        let Some((id, uri, pos)) = Self::position_request(content) else {
            return false;
        };
        let Some(text) = self.documents.get(&uri).map(|d| d.text.clone()) else {
            return false;
        };
        if !selectors_yml::selector_completion_applies(&uri, &text, pos) {
            return false;
        }
        let ctx = self.ctx_for(&uri);
        let open = Open::of(self.documents.texts());
        self.offload_blocking(id, "textDocument/completion", move || {
            Reply::result(&selectors::completion(&ctx, open, &uri, &text, pos))
        });
        true
    }

    /// The selectors.yml lenses of a document, for the offloaded lens task.
    pub(super) fn selectors_open(&self) -> Open {
        Open::of(self.documents.texts())
    }
}
