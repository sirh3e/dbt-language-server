//! `textDocument/semanticTokens/{full,range}`: the refined SQL token
//! stream for dbt SQL, the highlight captures for YAML documents
//! (`server/semantic_yaml.rs`, specs/highlighting §12).

use cogitator_analysis::features;
use cogitator_lsp::{Envelope, SemanticTokens};
use serde::Deserialize;

use super::Dispatcher;

#[derive(Deserialize)]
struct Line {
    line: u32,
}

#[derive(Deserialize)]
struct Lines {
    start: Line,
    end: Line,
}

#[derive(Deserialize)]
struct Doc {
    uri: String,
}

#[derive(Deserialize)]
struct Params {
    #[serde(rename = "textDocument")]
    text_document: Doc,
    range: Option<Lines>,
}

impl Dispatcher {
    pub(super) fn semantic_tokens(&mut self, content: &[u8], ranged: bool) {
        let method = if ranged {
            "textDocument/semanticTokens/range"
        } else {
            "textDocument/semanticTokens/full"
        };
        let Some((id, params)) = serde_json::from_slice::<Envelope<Params>>(content)
            .ok()
            .and_then(|r| Some((r.id, r.params?)))
            .filter(|(_, p)| !ranged || p.range.is_some())
        else {
            self.invalid_params(content, method);
            return;
        };
        let uri = params.text_document.uri;
        let lines = params
            .range
            .map(|r| (r.start.line, r.end.line.max(r.start.line)));
        if crate::server::semantic_yaml::is_yaml(&uri) {
            let snap = self.snapshot_for(&uri);
            let derived = self.derived.clone();
            self.offload_blocking(id, method, move || {
                let tokens = crate::server::semantic_yaml::tokens(&snap, &derived, &uri, lines);
                crate::server::reply::Reply::result(&tokens)
            });
            return;
        }
        let full = self
            .documents
            .get(&uri)
            .map(features::semantic_tokens)
            .unwrap_or(SemanticTokens { data: Vec::new() });
        let tokens = match lines {
            Some((start, end)) => features::tokens_in_lines(&full, start, end),
            None => full,
        };
        self.send(&cogitator_lsp::ResultResponse::new(id, tokens));
    }
}
