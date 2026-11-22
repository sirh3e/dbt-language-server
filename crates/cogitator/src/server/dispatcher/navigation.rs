//! Project-wide navigation requests that read other files or can refuse:
//! references (every dbt kind, `includeDeclaration` honoured), rename and
//! prepareRename (models and macros), and the call hierarchy's prepare
//! and outgoing calls. File-reading work is offloaded (05 §6.1).

use cogitator_analysis::features;
use cogitator_lsp::{error_codes, Envelope, ErrorResponse, ResultResponse};

use super::Dispatcher;
use crate::server::reply::Reply;

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
            let locations = features::references_at(
                &snap.ctx,
                snap.doc.as_deref(),
                &uri,
                position,
                include_declaration,
            );
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
        self.offload_blocking(
            req.id,
            "textDocument/rename",
            move || match features::rename_at(
                &snap.ctx,
                snap.doc.as_deref(),
                &uri,
                position,
                &new_name,
            ) {
                Ok(edit) => Reply::result(&edit),
                Err(reason) => Reply::error(error_codes::REQUEST_FAILED, reason),
            },
        );
    }

    /// A ref or a macro of this project prepares; an installed package's
    /// macro is refused with the reason, anything else answers null.
    pub(super) fn prepare_rename(&mut self, content: &[u8]) {
        let Some((id, uri, pos)) = Self::position_request(content) else {
            self.invalid_params(content, "textDocument/prepareRename");
            return;
        };
        let ctx = self.ctx_for(&uri);
        let answer = match self.documents.get(&uri) {
            Some(doc) => features::prepare_rename_at(&ctx, doc, &uri, pos),
            None => Ok(None),
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
