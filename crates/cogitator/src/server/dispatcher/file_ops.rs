//! The shell of the workspace file operations and of declaration:
//! willRenameFiles (the ref and YAML edits, offloaded: dependents are
//! read from disk), willDeleteFiles (an empty edit and one warning
//! naming the dependents left dangling), the did* notifications (a
//! context refresh of every root they touch, so the graph, the lenses
//! — "Extract schema YAML" on a new model — and the diagnostics follow
//! without a didChangeWatchedFiles registration), textDocument/
//! declaration, and definition on a properties-YAML column (offloaded:
//! the model's SQL is read).

use std::collections::BTreeSet;

use cogitator_analysis::features;
use cogitator_lsp::{
    message_type, Envelope, FileUrisParams, Position, RenameFilesParams, ResultResponse,
    WorkspaceEdit,
};

use super::Dispatcher;
use crate::server::reply::Reply;
use crate::server::snapshot::Snapshot;

impl Dispatcher {
    pub(super) fn will_rename_files(&mut self, content: &[u8]) {
        let Ok(req) = serde_json::from_slice::<Envelope<RenameFilesParams>>(content) else {
            self.invalid_params(content, "workspace/willRenameFiles");
            return;
        };
        let Some(params) = req.params else {
            self.invalid_params(content, "workspace/willRenameFiles");
            return;
        };
        let renames: Vec<(String, String)> = params
            .files
            .into_iter()
            .map(|f| (f.old_uri.into_owned(), f.new_uri.into_owned()))
            .collect();
        let first = renames
            .first()
            .map(|(old, _)| old.clone())
            .unwrap_or_default();
        let snap = self.snapshot_for(&first);
        self.offload_blocking(req.id, "workspace/willRenameFiles", move || {
            let read = |p: &std::path::Path| std::fs::read_to_string(p).ok();
            Reply::result(&features::will_rename_files(&snap.ctx, &renames, &read))
        });
    }

    pub(super) fn will_delete_files(&mut self, content: &[u8]) {
        let Ok(req) = serde_json::from_slice::<Envelope<FileUrisParams>>(content) else {
            self.invalid_params(content, "workspace/willDeleteFiles");
            return;
        };
        let Some(params) = req.params else {
            self.invalid_params(content, "workspace/willDeleteFiles");
            return;
        };
        let uris: Vec<String> = params
            .files
            .into_iter()
            .map(|f| f.uri.into_owned())
            .collect();
        let ctx = self.ctx_for(uris.first().map_or("", String::as_str));
        // the graph alone answers: no file is read
        if let Some(text) =
            features::delete_warning(&features::deleted_with_dependents(&ctx, &uris))
        {
            self.show_message(message_type::WARNING, text);
        }
        let empty = WorkspaceEdit {
            document_changes: Vec::new(),
        };
        self.send(&ResultResponse::new(req.id, empty));
    }

    /// didCreate/didRename/didDelete: refresh every root a touched dbt
    /// file belongs to (a rename refreshes both ends).
    pub(super) fn did_change_files(&mut self, content: &[u8], method: &str) {
        let uris: Vec<String> = if method == "workspace/didRenameFiles" {
            serde_json::from_slice::<Envelope<RenameFilesParams>>(content)
                .ok()
                .and_then(|r| r.params)
                .map(|p| {
                    p.files
                        .into_iter()
                        .flat_map(|f| [f.old_uri.into_owned(), f.new_uri.into_owned()])
                        .collect()
                })
                .unwrap_or_default()
        } else {
            serde_json::from_slice::<Envelope<FileUrisParams>>(content)
                .ok()
                .and_then(|r| r.params)
                .map(|p| p.files.into_iter().map(|f| f.uri.into_owned()).collect())
                .unwrap_or_default()
        };
        let roots: BTreeSet<String> = uris
            .iter()
            .filter(|u| features::touches_project(u))
            .map(|u| self.refresh_root_for(u))
            .collect();
        tracing::debug!(%method, files = uris.len(), roots = roots.len(), "file operation");
        for root in roots {
            let _ = self.refresh_tx.send(root);
        }
    }

    pub(super) fn declaration(&mut self, content: &[u8]) {
        self.respond_position(content, "textDocument/declaration", |ctx, doc, uri, pos| {
            doc.map(|d| features::declaration(ctx, d, uri, pos))
                .unwrap_or_else(|| cogitator_lsp::Location {
                    uri: uri.to_string(),
                    range: cogitator_lsp::Range::zero_width(pos),
                })
        });
    }

    /// Definition on a properties-YAML column entry: where the model's
    /// SQL produces it. `false`: not such a position (the caller goes on).
    pub(super) fn yaml_column_definition(
        &mut self,
        id: i64,
        uri: &str,
        pos: Position,
        snap: &Snapshot,
    ) -> bool {
        let Some(doc) = snap.doc.clone() else {
            return false;
        };
        let Some((model, column)) = features::yaml_column_target(&snap.ctx, &doc, uri, pos) else {
            return false;
        };
        let ctx = snap.ctx.clone();
        // the model's own buffer, when open, wins over its file
        let open = ctx
            .model_detail_map
            .get(&model)
            .map(|d| cogitator_lsp::fs_path_to_uri(&d.uri))
            .and_then(|u| self.documents.get_arc(&u).map(|d| (u, d)));
        self.offload_blocking(id, "textDocument/definition", move || {
            let read = |p: &std::path::Path| std::fs::read_to_string(p).ok();
            let open = open.as_ref().map(|(u, d)| (u.as_str(), &*d.text));
            match features::yaml_column_definition(&ctx, &model, &column, open, &read) {
                Some(location) => Reply::result(&location),
                None => Reply::result(&()),
            }
        });
        true
    }
}
