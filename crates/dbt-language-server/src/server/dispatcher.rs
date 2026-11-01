//! Method dispatch (specs/rust-port/01 §3). One span per message (05 §6.3);
//! unknown methods ignored; parse failures logged and dropped, never an
//! error response — faithful to Go.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use dbt_ls_analysis::{features, DbtContext, Documents};
use dbt_ls_lsp::{
    rpc, DidChangeParams, DidOpenParams, DidSaveParams, Envelope, ExecuteCommandParams,
    HoverResult, InitializeParams, Position, PositionParams, RequestId, ResultResponse,
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
            "textDocument/didOpen" => self.did_open(content),
            "textDocument/didSave" => self.did_save(content),
            "textDocument/didChange" => self.did_change(content),
            "textDocument/hover" => self.hover(content),
            "textDocument/definition" => self.definition(content),
            "textDocument/completion" => self.completion(content),
            "workspace/executeCommand" => self.execute_command(content),
            "shutdown" => self.shutdown(content),
            "exit" => {
                tracing::info!("exit notification");
                crate::observability::flush();
                std::process::exit(0);
            }
            _ => {}
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
        self.spawn_fusion(uri);
    }

    fn did_save(&mut self, content: &[u8]) {
        let Ok(req) = serde_json::from_slice::<Envelope<DidSaveParams>>(content) else {
            return;
        };
        let Some(params) = req.params else { return };
        let uri = params.text_document.uri.into_owned();
        tracing::debug!(%uri, "saved");
        let _ = self.refresh_tx.send(self.root_path.clone());
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
        let Some((id, uri, pos)) = Self::position_request(content) else {
            return;
        };
        let ctx = self.snapshot();
        let contents = self
            .documents
            .get(&uri)
            .map(|doc| features::hover(&ctx, doc, pos))
            .unwrap_or_default(); // Q8: unknown doc → empty hover
        self.send(&ResultResponse::new(id, HoverResult { contents }));
    }

    fn definition(&mut self, content: &[u8]) {
        let Some((id, uri, pos)) = Self::position_request(content) else {
            return;
        };
        let ctx = self.snapshot();
        let location = match self.documents.get(&uri) {
            Some(doc) => features::definition(&ctx, doc, &uri, pos),
            None => dbt_ls_lsp::Location {
                uri: uri.clone(),
                range: dbt_ls_lsp::Range::zero_width(pos),
            },
        };
        self.send(&ResultResponse::new(id, location));
    }

    fn completion(&mut self, content: &[u8]) {
        let Some((id, uri, pos)) = Self::position_request(content) else {
            return;
        };
        let ctx = self.snapshot();
        let items = self
            .documents
            .get(&uri)
            .map(|doc| features::completion(&ctx, doc, pos))
            .unwrap_or_default();
        self.send(&ResultResponse::new(id, items));
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
