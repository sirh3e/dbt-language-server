//! Method dispatch (specs/rust-port/01 §3). One span per message (05 §6.3);
//! unknown methods ignored; parse failures logged and dropped, never an
//! error response — faithful to Go.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use dbt_ls_analysis::{features, yaml_features, DbtContext, Documents};
use dbt_ls_lsp::{
    error_codes, rpc, Diagnostic, DidChangeParams, DidOpenParams, DidSaveParams, Envelope,
    ErrorResponse, ExecuteCommandParams, FullDocumentDiagnosticReport, HoverResult,
    InitializeParams, MaybeId, Position, PositionParams, RequestId, ResultResponse,
    SelectionRangeParams, WorkspaceSymbolParams,
};
use serde::Serialize;
use tokio::sync::{mpsc, watch};
use tokio::task::AbortHandle;

use super::fusion;

pub struct Dispatcher {
    documents: Documents,
    write_tx: mpsc::UnboundedSender<Vec<u8>>,
    ctx_rx: watch::Receiver<Arc<DbtContext>>,
    refresh_tx: mpsc::UnboundedSender<String>,
    root_path: String,
    fusion_path: String,
    fusion_enabled: Arc<AtomicBool>,
    fusion_jobs: HashMap<String, AbortHandle>,
    scratch: Vec<u8>,
}

impl Dispatcher {
    pub fn new(
        write_tx: mpsc::UnboundedSender<Vec<u8>>,
        ctx_rx: watch::Receiver<Arc<DbtContext>>,
        refresh_tx: mpsc::UnboundedSender<String>,
        fusion_path: String,
        fusion_enabled: Arc<AtomicBool>,
    ) -> Self {
        Dispatcher {
            documents: Documents::default(),
            write_tx,
            ctx_rx,
            refresh_tx,
            root_path: String::new(),
            fusion_path,
            fusion_enabled,
            fusion_jobs: HashMap::new(),
            scratch: Vec::new(),
        }
    }

    fn snapshot(&self) -> Arc<DbtContext> {
        self.ctx_rx.borrow().clone()
    }

    fn send<T: Serialize>(&mut self, msg: &T) {
        let mut out = Vec::new();
        if rpc::encode_frame(msg, &mut self.scratch, &mut out).is_ok() {
            let _ = self.write_tx.send(out);
        }
    }

    pub async fn handle(&mut self, content: &[u8]) {
        let method = match rpc::decode_method(content) {
            Ok(m) => m,
            Err(err) => {
                tracing::debug!(%err, "undecodable message");
                return;
            }
        };
        let span = tracing::debug_span!("lsp_message", %method);
        let _enter = span.enter();

        match method.as_str() {
            "initialize" => self.initialize(content),
            "initialized" => tracing::debug!("client initialized"),
            "textDocument/didOpen" => self.did_open(content),
            "textDocument/didSave" => self.did_save(content),
            "textDocument/didChange" => self.did_change(content),
            "textDocument/didClose" => self.did_close(content),
            "textDocument/hover" => self.hover(content),
            "textDocument/definition" => self.definition(content),
            "textDocument/completion" => self.completion(content),
            "completionItem/resolve" => self.resolve_completion(content),
            "textDocument/references" => self.references(content),
            "textDocument/documentSymbol" => self.document_symbol(content),
            "textDocument/documentHighlight" => self.document_highlight(content),
            "textDocument/foldingRange" => self.folding_range(content),
            "textDocument/selectionRange" => self.selection_range(content),
            "textDocument/semanticTokens/full" => self.semantic_tokens(content),
            "textDocument/signatureHelp" => self.signature_help(content),
            "textDocument/diagnostic" => self.pull_diagnostics(content),
            "workspace/symbol" => self.workspace_symbol(content),
            "workspace/didChangeWatchedFiles" => {
                // best-effort: clients only send this after dynamic
                // registration, which this server does not request yet
                let _ = self.refresh_tx.send(self.root_path.clone());
            }
            "$/cancelRequest" => {
                // accepted per protocol; handlers are sub-millisecond and
                // synchronous, so there is nothing in flight to cancel
                tracing::trace!("cancel request acknowledged");
            }
            "workspace/executeCommand" => self.execute_command(content),
            "shutdown" => self.shutdown(content),
            "exit" => {
                tracing::info!("exit notification");
                crate::observability::flush();
                std::process::exit(0);
            }
            other if other.starts_with("$/") => {} // optional notifications
            _ => {
                // unknown REQUESTS get MethodNotFound; notifications stay
                // silently ignored (01 §3 behavior for known-but-unhandled)
                if let Ok(MaybeId { id: Some(id) }) = serde_json::from_slice::<MaybeId>(content) {
                    let response = ErrorResponse::new(
                        id,
                        error_codes::METHOD_NOT_FOUND,
                        format!("method not found: {method}"),
                    );
                    self.send(&response);
                }
            }
        }
    }

    /// Reply InvalidParams when a request (id present) has undecodable params.
    fn invalid_params(&mut self, content: &[u8], method: &str) {
        if let Ok(MaybeId { id: Some(id) }) = serde_json::from_slice::<MaybeId>(content) {
            let response = ErrorResponse::new(
                id,
                error_codes::INVALID_PARAMS,
                format!("invalid params for {method}"),
            );
            self.send(&response);
        }
    }

    fn initialize(&mut self, content: &[u8]) {
        let Ok(req) = serde_json::from_slice::<Envelope<InitializeParams>>(content) else {
            return;
        };
        let params = req.params.unwrap_or_default();
        if let Some(info) = &params.client_info {
            tracing::info!(client = %info.name, version = %info.version, "connected");
        }
        self.root_path = params.root_path.map(|p| p.into_owned()).unwrap_or_default();
        let response = dbt_ls_lsp::initialize_response(req.id, crate::VERSION);
        self.send(&response);
        // first refresh so hover works before the first save (05 §6.2)
        let _ = self.refresh_tx.send(self.root_path.clone());
    }

    fn did_open(&mut self, content: &[u8]) {
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
        self.publish_static_diagnostics(&uri);
        self.spawn_fusion(uri);
    }

    fn did_close(&mut self, content: &[u8]) {
        let Ok(req) = serde_json::from_slice::<Envelope<DidSaveParams>>(content) else {
            return;
        };
        let Some(params) = req.params else { return };
        self.documents.close(&params.text_document.uri);
        tracing::debug!(uri = %params.text_document.uri, "closed");
    }

    fn did_save(&mut self, content: &[u8]) {
        let Ok(req) = serde_json::from_slice::<Envelope<DidSaveParams>>(content) else {
            return;
        };
        let Some(params) = req.params else { return };
        let uri = params.text_document.uri.into_owned();
        tracing::debug!(%uri, "saved");
        let _ = self.refresh_tx.send(self.root_path.clone());
        self.publish_static_diagnostics(&uri);
        self.spawn_fusion(uri);
    }

    fn did_change(&mut self, content: &[u8]) {
        let Ok(req) = serde_json::from_slice::<Envelope<DidChangeParams>>(content) else {
            return;
        };
        let Some(params) = req.params else { return };
        let dialect = self.snapshot().dialect.clone();
        self.documents
            .change(&params.text_document.uri, &params.content_changes, &dialect);
    }

    /// Composition point for every position-shaped request: parse → snapshot
    /// → pure feature function → serialize (functional core / imperative
    /// shell, 05 §3). Bad params on a request answer InvalidParams.
    fn respond_position<R: serde::Serialize>(
        &mut self,
        content: &[u8],
        method: &str,
        feature: impl FnOnce(&DbtContext, Option<&dbt_ls_analysis::Document>, &str, Position) -> R,
    ) {
        let Some((id, uri, pos)) = Self::position_request(content) else {
            self.invalid_params(content, method);
            return;
        };
        let ctx = self.snapshot();
        let result = feature(&ctx, self.documents.get(&uri), &uri, pos);
        self.send(&ResultResponse::new(id, result));
    }

    fn position_request<'a>(content: &'a [u8]) -> Option<(i64, String, Position)> {
        let req = serde_json::from_slice::<Envelope<PositionParams<'a>>>(content).ok()?;
        let params = req.params?;
        Some((
            req.id,
            params.text_document.uri.into_owned(),
            params.position,
        ))
    }

    fn hover(&mut self, content: &[u8]) {
        self.respond_position(content, "textDocument/hover", |ctx, doc, _, pos| {
            HoverResult {
                // Q8: unknown doc → empty hover
                contents: doc
                    .map(|d| features::hover(ctx, d, pos))
                    .unwrap_or_default(),
            }
        });
    }

    fn definition(&mut self, content: &[u8]) {
        self.respond_position(
            content,
            "textDocument/definition",
            |ctx, doc, uri, pos| match doc {
                Some(doc) => features::definition(ctx, doc, uri, pos),
                None => dbt_ls_lsp::Location {
                    uri: uri.to_string(),
                    range: dbt_ls_lsp::Range::zero_width(pos),
                },
            },
        );
    }

    fn completion(&mut self, content: &[u8]) {
        self.respond_position(content, "textDocument/completion", |ctx, doc, uri, pos| {
            match doc {
                // dbt YAML files get schema-key completion (08 §2.6 v1)
                Some(d) if super::diagnostics::is_yaml(uri) => {
                    yaml_features::yaml_completion(uri, &d.text, pos)
                }
                Some(d) => features::completion(ctx, d, pos),
                None => Vec::new(),
            }
        });
    }

    fn execute_command(&mut self, content: &[u8]) {
        let Ok(req) = serde_json::from_slice::<Envelope<ExecuteCommandParams>>(content) else {
            return;
        };
        let Some(params) = req.params else { return };
        if params.command != "dbt.goToSchema" {
            return;
        }
        // untyped arguments (01 §3.4); malformed → request dropped like Go
        let Some(arg) = params.arguments.first().and_then(|a| a.as_object()) else {
            return;
        };
        let uri = arg
            .get("uri")
            .and_then(|u| u.as_str())
            .unwrap_or_default()
            .to_string();
        let pos = arg
            .get("position")
            .map(|p| Position {
                line: p.get("line").and_then(|v| v.as_f64()).unwrap_or(0.0) as u32,
                character: p.get("character").and_then(|v| v.as_f64()).unwrap_or(0.0) as u32,
            })
            .unwrap_or_default();

        let ctx = self.snapshot();
        let result = features::go_to_schema(&ctx, self.documents.get(&uri), &uri, pos);
        self.send(&ResultResponse::new(req.id, result));
    }

    fn shutdown(&mut self, content: &[u8]) {
        let Ok(req) = serde_json::from_slice::<RequestId>(content) else {
            return;
        };
        tracing::info!("shutdown request");
        // bare response without a result field — Go's exact shape (01 §3.3)
        self.send(&dbt_ls_lsp::Response::new(req.id));
    }

    fn resolve_completion(&mut self, content: &[u8]) {
        let Ok(req) = serde_json::from_slice::<MaybeId>(content) else {
            return;
        };
        let Some(id) = req.id else { return };
        let Ok(item) = serde_json::from_slice::<serde_json::Value>(content) else {
            return;
        };
        let mut item = item.get("params").cloned().unwrap_or(item);
        let ctx = self.snapshot();
        if let Some(docs) = item
            .get("data")
            .and_then(|data| features::resolve_completion_documentation(&ctx, data))
        {
            if let Some(map) = item.as_object_mut() {
                map.insert("documentation".into(), serde_json::Value::String(docs));
            }
        }
        self.send(&ResultResponse::new(id, item));
    }

    fn references(&mut self, content: &[u8]) {
        self.respond_position(content, "textDocument/references", features::references);
    }

    fn document_symbol(&mut self, content: &[u8]) {
        self.respond_position(content, "textDocument/documentSymbol", |_, doc, uri, _| {
            doc.map(|d| features::document_symbols(d, uri))
                .unwrap_or_default()
        });
    }

    fn document_highlight(&mut self, content: &[u8]) {
        self.respond_position(
            content,
            "textDocument/documentHighlight",
            |_, doc, _, pos| {
                doc.map(|d| features::document_highlights(d, pos))
                    .unwrap_or_default()
            },
        );
    }

    fn folding_range(&mut self, content: &[u8]) {
        self.respond_position(content, "textDocument/foldingRange", |_, doc, _, _| {
            doc.map(features::folding_ranges).unwrap_or_default()
        });
    }

    fn selection_range(&mut self, content: &[u8]) {
        let Ok(req) = serde_json::from_slice::<Envelope<SelectionRangeParams>>(content) else {
            self.invalid_params(content, "textDocument/selectionRange");
            return;
        };
        let Some(params) = req.params else {
            self.invalid_params(content, "textDocument/selectionRange");
            return;
        };
        let uri = params.text_document.uri.into_owned();
        let ranges = self
            .documents
            .get(&uri)
            .map(|doc| features::selection_ranges(doc, &params.positions))
            .unwrap_or_default();
        self.send(&ResultResponse::new(req.id, ranges));
    }

    fn semantic_tokens(&mut self, content: &[u8]) {
        self.respond_position(
            content,
            "textDocument/semanticTokens/full",
            |_, doc, _, _| {
                doc.map(features::semantic_tokens)
                    .unwrap_or(dbt_ls_lsp::SemanticTokens { data: Vec::new() })
            },
        );
    }

    fn signature_help(&mut self, content: &[u8]) {
        self.respond_position(content, "textDocument/signatureHelp", |ctx, doc, _, pos| {
            doc.and_then(|d| features::signature_help(ctx, d, pos))
        });
    }

    fn workspace_symbol(&mut self, content: &[u8]) {
        let Ok(req) = serde_json::from_slice::<Envelope<WorkspaceSymbolParams>>(content) else {
            self.invalid_params(content, "workspace/symbol");
            return;
        };
        let query = req
            .params
            .and_then(|p| p.query.map(|q| q.into_owned()))
            .unwrap_or_default();
        let ctx = self.snapshot();
        self.send(&ResultResponse::new(
            req.id,
            features::workspace_symbols(&ctx, &query),
        ));
    }

    /// textDocument/diagnostic (pull): same static set the push pipeline uses.
    fn pull_diagnostics(&mut self, content: &[u8]) {
        let Some((id, uri, _)) = Self::position_request(content) else {
            self.invalid_params(content, "textDocument/diagnostic");
            return;
        };
        let items = self.static_diagnostics(&uri);
        self.send(&ResultResponse::new(
            id,
            FullDocumentDiagnosticReport {
                kind: "full",
                items,
            },
        ));
    }

    /// Diagnostics for one open doc — see server::diagnostics.
    fn static_diagnostics(&self, uri: &str) -> Vec<Diagnostic> {
        super::diagnostics::static_diagnostics(&self.snapshot(), &self.documents, uri)
    }

    /// Push pipeline (didOpen/didSave). Fusion owns publishes when enabled
    /// (07 §3 precedence) — never double-publish.
    fn publish_static_diagnostics(&mut self, uri: &str) {
        if self.fusion_enabled.load(Ordering::Relaxed) {
            return;
        }
        if self.documents.get(uri).is_none() {
            return;
        }
        let items = self.static_diagnostics(uri);
        let note = dbt_ls_lsp::publish_diagnostics(uri.to_string(), items);
        self.send(&note);
    }

    /// Fusion compile per 04 §2, async + cancellable per URI (fixes Q9).
    fn spawn_fusion(&mut self, uri: String) {
        if !self.fusion_enabled.load(Ordering::Relaxed) {
            return;
        }
        if let Some(previous) = self.fusion_jobs.remove(&uri) {
            previous.abort();
        }
        let ctx = self.snapshot();
        let job = fusion::FusionJob {
            fusion_path: self.fusion_path.clone(),
            project_name: ctx.project.name.clone(),
            uri: uri.clone(),
            write_tx: self.write_tx.clone(),
        };
        let handle = tokio::spawn(job.run());
        self.fusion_jobs.insert(uri, handle.abort_handle());
    }
}
