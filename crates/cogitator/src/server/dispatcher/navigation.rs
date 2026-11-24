//! Project-wide navigation requests that read other files or can refuse:
//! references (every dbt kind, `includeDeclaration` honoured), rename and
//! prepareRename (file-local names, vars, columns, macros and models), and
//! the call hierarchy's prepare and outgoing calls. File-reading work is
//! offloaded (05 §6.1).

use cogitator_analysis::{features, Document};
use cogitator_lsp::{error_codes, Envelope, ErrorResponse, ResultResponse};

use super::Dispatcher;
use crate::server::reply::Reply;

/// The file behind `uri` parsed from disk when the editor has not opened
/// it, so a position in an unopened file (a macro picked from a list)
/// still names what is there. Offloaded handlers only: it reads a file.
fn on_disk(snap: &crate::server::snapshot::Snapshot, uri: &str) -> Option<Document> {
    if snap.doc.is_some() {
        return None;
    }
    Document::from_disk(uri, &snap.ctx.dialect)
}

impl Dispatcher {
    pub(super) fn references(&mut self, content: &[u8]) {
        let Ok(req) = serde_json::from_slice::<Envelope<cogitator_lsp::ReferenceParams>>(content)
        else {
            self.invalid_params(content, "textDocument/references");
            return;
        };
        let Some(params) = req.params else {
            self.invalid_params(content, "textDocument/references");
            return;
        };
        let uri = params.text_document.uri.into_owned();
        let position = params.position;
        let include_declaration = params.context.include_declaration;
        let snap = self.snapshot_for(&uri);
        self.offload_blocking(req.id, "textDocument/references", move || {
            let disk = on_disk(&snap, &uri);
            let doc = snap.doc.as_deref().or(disk.as_ref());
            // a name local to the file first, then every dbt kind
            let locations =
                features::local_references(&snap.ctx, doc, &uri, position, include_declaration)
                    .unwrap_or_else(|| {
                        features::references_at(&snap.ctx, doc, &uri, position, include_declaration)
                    });
            Reply::result(&locations)
        });
    }

    pub(super) fn rename(&mut self, content: &[u8]) {
        let Ok(req) = serde_json::from_slice::<Envelope<cogitator_lsp::RenameParams>>(content)
        else {
            self.invalid_params(content, "textDocument/rename");
            return;
        };
        let Some(params) = req.params else {
            self.invalid_params(content, "textDocument/rename");
            return;
        };
        let uri = params.text_document.uri.into_owned();
        let new_name = params.new_name.into_owned();
        let position = params.position;
        let snap = self.snapshot_for(&uri);
        self.offload_blocking(req.id, "textDocument/rename", move || {
            let disk = on_disk(&snap, &uri);
            let doc = snap.doc.as_deref().or(disk.as_ref());
            let read = |p: &std::path::Path| std::fs::read_to_string(p).ok();
            // local name → var → column → macro → model: the first that names the cursor
            let answer = None
                .or_else(|| {
                    features::rename_local_at(&snap.ctx, doc, &uri, position, &new_name, &read)
                })
                .or_else(|| {
                    features::rename_var_at(&snap.ctx, doc, &uri, position, &new_name, &read)
                })
                .or_else(|| {
                    features::rename_column_at(&snap.ctx, doc, &uri, position, &new_name, &read)
                })
                .map(|edit| edit.map(Some))
                .unwrap_or_else(|| features::rename_at(&snap.ctx, doc, &uri, position, &new_name));
            match answer {
                Ok(edit) => Reply::result(&edit),
                Err(reason) => Reply::error(error_codes::REQUEST_FAILED, reason),
            }
        });
    }

    /// A var (call or `vars:` key), a model column, a ref or a macro of
    /// this project prepares; an installed package's macro is refused with the reason,
    /// anything else answers null.
    pub(super) fn prepare_rename(&mut self, content: &[u8]) {
        let Some((id, uri, pos)) = Self::position_request(content) else {
            self.invalid_params(content, "textDocument/prepareRename");
            return;
        };
        let ctx = self.ctx_for(&uri);
        let doc = self.documents.get(&uri);
        // a local name, a var call or `vars:` key, a column, then macros and models
        let local = features::prepare_rename_local(&ctx, doc, &uri, pos);
        let early = None
            .or_else(|| features::prepare_rename_var(&ctx, doc, &uri, pos))
            .or_else(|| features::prepare_rename_column(&ctx, doc, &uri, pos));
        let answer = match (local, early, doc) {
            (Some(local), ..) => local.map(Some),
            (None, Some(early), _) => Ok(Some(early)),
            (None, None, Some(doc)) => features::prepare_rename_at(&ctx, doc, &uri, pos),
            (None, None, None) => Ok(None),
        };
        match answer {
            Ok(result) => self.send(&ResultResponse::new(id, result)),
            Err(reason) => self.send(&ErrorResponse::new(id, error_codes::REQUEST_FAILED, reason)),
        }
    }

    /// prepare answers `[item]` for a macro call or definition header
    /// under the cursor, null anywhere else.
    pub(super) fn prepare_call_hierarchy(&mut self, content: &[u8]) {
        self.respond_position(
            content,
            "textDocument/prepareCallHierarchy",
            |ctx, doc, uri, pos| {
                doc.and_then(|d| features::prepare_call_hierarchy(ctx, d, uri, pos))
                    .map(|item| vec![item])
            },
        );
    }

    pub(super) fn call_hierarchy_outgoing(&mut self, content: &[u8]) {
        let Ok(req) =
            serde_json::from_slice::<Envelope<cogitator_lsp::CallHierarchyCallsParams>>(content)
        else {
            self.invalid_params(content, "callHierarchy/outgoingCalls");
            return;
        };
        let Some(params) = req.params else {
            self.invalid_params(content, "callHierarchy/outgoingCalls");
            return;
        };
        let ctx = self.ctx_for(&params.item.uri);
        // offloaded: the item's file is read from disk
        self.offload_blocking(req.id, "callHierarchy/outgoingCalls", move || {
            Reply::result(&features::macro_outgoing_calls(&ctx, &params.item))
        });
    }
}
