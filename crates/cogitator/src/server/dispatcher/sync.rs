//! Document sync + the push diagnostics pipeline: didOpen/didChange/
//! didSave/didClose, the fs-free publish path, and per-URI fusion
//! compiles. State mutation stays on the dispatcher task (05 §6.2).

use std::sync::atomic::Ordering;

use cogitator_analysis::features;

use cogitator_lsp::{Diagnostic, DidChangeParams, DidOpenParams, DidSaveParams, Envelope};

use super::fusion;
use super::Dispatcher;

impl Dispatcher {
    pub(super) fn did_open(&mut self, content: &[u8]) {
        let Ok(req) = serde_json::from_slice::<Envelope<DidOpenParams>>(content) else {
            tracing::debug!("textDocument/didOpen: bad params");
            return;
        };
        let Some(params) = req.params else { return };
        let uri = params.text_document.uri.into_owned();
        let dialect = self.snapshot().dialect.clone();
        self.documents
            .open(uri.clone(), &params.text_document.text, &dialect);
        tracing::debug!(%uri, "opened");
        let _ = self.refresh_tx.send(self.root_path.clone());
        if self.is_config_uri(&uri) {
            let _ = self.config_poke_tx.send(());
        }
        self.publish_static_diagnostics(&uri);
        self.spawn_fusion(uri);
    }

    pub(super) fn did_close(&mut self, content: &[u8]) {
        let Ok(req) = serde_json::from_slice::<Envelope<DidSaveParams>>(content) else {
            return;
        };
        let Some(params) = req.params else { return };
        self.documents.close(&params.text_document.uri);
        tracing::debug!(uri = %params.text_document.uri, "closed");
    }

    pub(super) fn did_save(&mut self, content: &[u8]) {
        let Ok(req) = serde_json::from_slice::<Envelope<DidSaveParams>>(content) else {
            return;
        };
        let Some(params) = req.params else { return };
        let uri = params.text_document.uri.into_owned();
        tracing::debug!(%uri, "saved");
        let _ = self.refresh_tx.send(self.root_path.clone());
        if self.is_config_uri(&uri) {
            let _ = self.config_poke_tx.send(());
        }
        self.publish_static_diagnostics(&uri);
        self.spawn_fusion(uri);
    }

    pub(super) fn did_change(&mut self, content: &[u8]) {
        let Ok(req) = serde_json::from_slice::<Envelope<DidChangeParams>>(content) else {
            return;
        };
        let Some(params) = req.params else { return };
        let dialect = self.snapshot().dialect.clone();
        self.documents
            .change(&params.text_document.uri, &params.content_changes, &dialect);
    }

    /// Diagnostics for one open doc — see server::diagnostics.
    pub(super) fn static_diagnostics(&self, uri: &str) -> Vec<Diagnostic> {
        let lints =
            features::lint::LintOptions::from_flags(self.toggles.lints.load(Ordering::Relaxed));
        // push stays inline and lineage-free; the pull path adds the
        // lineage-backed check
        let ctx = self.snapshot();
        let mut items = crate::server::diagnostics::static_diagnostics(
            &ctx,
            self.documents.get(uri),
            uri,
            lints,
            None,
        );
        // artifacts come from the LAST loaded set — no fs on this task
        // (the offloaded lens/pull paths keep it warm)
        if let Some(root) = ctx.project_root.as_deref() {
            let set = self.derived.artifacts_cached(root);
            items.extend(features::artifact_diagnostics(&ctx, uri, &set));
        }
        items
    }

    /// Push pipeline (didOpen/didSave). Fusion owns publishes when enabled
    /// (07 §3 precedence) — never double-publish.
    pub(super) fn publish_static_diagnostics(&mut self, uri: &str) {
        if self.fusion_enabled.load(Ordering::Relaxed) {
            return;
        }
        if self.documents.get(uri).is_none() {
            return;
        }
        let items = self.static_diagnostics(uri);
        let note = cogitator_lsp::publish_diagnostics(uri.to_string(), items);
        self.send(&note);
    }

    /// Fusion compile per 04 §2, async + cancellable per URI (fixes Q9).
    pub(super) fn spawn_fusion(&mut self, uri: String) {
        if !self.fusion_enabled.load(Ordering::Relaxed) {
            return;
        }
        // completed jobs never removed themselves: drop finished handles so
        // the map stays bounded by the number of URIs actually in flight
        self.fusion_jobs.retain(|_, handle| !handle.is_finished());
        if let Some(previous) = self.fusion_jobs.remove(&uri) {
            previous.abort();
        }
        let ctx = self.snapshot();
        let job = fusion::FusionJob {
            fusion_path: self.fusion_path.clone(),
            project_name: ctx.project.name.clone(),
            uri: uri.clone(),
            write_tx: self.link.writer(),
        };
        let handle = tokio::spawn(job.run());
        self.fusion_jobs.insert(uri, handle.abort_handle());
    }
}
