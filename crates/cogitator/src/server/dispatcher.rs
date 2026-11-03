//! Method dispatch (specs/rust-port/01 §3). One span per message (05 §6.3);
//! unknown methods ignored; parse failures logged and dropped, never an
//! error response — faithful to Go.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use cogitator_analysis::{features, yaml_features, DbtContext, Documents};

use cogitator_lsp::{
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
    /// id source for server→client requests (responses are ignored).
    server_request_ids: Arc<std::sync::atomic::AtomicI64>,
    /// Shared with the refresh task: initializationOptions and
    /// dbt.setTarget replace the launch-flag values here.
    profiles: Arc<std::sync::Mutex<super::ProfilesConfig>>,
    /// Client advertised window.workDoneProgress (set at initialize).
    progress_supported: Arc<AtomicBool>,
    /// What the server has been doing, for dbt.status.
    status: Arc<std::sync::Mutex<super::StatusInfo>>,
}

impl Dispatcher {
    // constructed exactly once, in server::run — the shared handles are
    // clearer spelled out than bundled into a one-off struct
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        documents: Documents,
        write_tx: mpsc::UnboundedSender<Vec<u8>>,
        ctx_rx: watch::Receiver<Arc<DbtContext>>,
        refresh_tx: mpsc::UnboundedSender<String>,
        fusion_path: String,
        fusion_enabled: Arc<AtomicBool>,
        profiles: Arc<std::sync::Mutex<super::ProfilesConfig>>,
        progress_supported: Arc<AtomicBool>,
        status: Arc<std::sync::Mutex<super::StatusInfo>>,
    ) -> Self {
        Dispatcher {
            documents,
            write_tx,
            ctx_rx,
            refresh_tx,
            root_path: String::new(),
            fusion_path,
            fusion_enabled,
            fusion_jobs: HashMap::new(),
            scratch: Vec::new(),
            server_request_ids: Arc::new(std::sync::atomic::AtomicI64::new(1)),
            profiles,
            progress_supported,
            status,
        }
    }

    fn snapshot(&self) -> Arc<DbtContext> {
        self.ctx_rx.borrow().clone()
    }

    fn show_message(&mut self, message_type: i32, message: impl Into<String>) {
        let note = cogitator_lsp::show_message(message_type, message);
        self.send(&note);
    }

    fn send_request<T: Serialize>(&mut self, method: &'static str, params: T) {
        let id = self.server_request_ids.fetch_add(1, Ordering::Relaxed);
        let request = cogitator_lsp::ServerRequest {
            jsonrpc: "2.0",
            id,
            method,
            params,
        };
        self.send(&request);
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
        if method.is_empty() {
            // a response to one of our server→client requests (id, no
            // method) — fire-and-forget: ignore instead of MethodNotFound
            tracing::trace!("client response ignored");
            return;
        }
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
            "textDocument/rename" => self.rename(content),
            "textDocument/codeLens" => self.code_lens(content),
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
        self.root_path = params.workspace_root().unwrap_or_default();
        self.progress_supported
            .store(params.work_done_progress(), Ordering::Relaxed);
        {
            let mut cfg = self.profiles.lock().expect("profiles config lock");
            if let Some(dir) = params.profiles_dir() {
                tracing::info!(dir, "profiles dir from initializationOptions");
                cfg.dir_override = dir.to_string();
            }
            if let Some(profile) = params.profile() {
                tracing::info!(profile, "profile from initializationOptions");
                cfg.profile = profile.to_string();
            }
            if let Some(target) = params.target() {
                tracing::info!(target, "target from initializationOptions");
                cfg.target = target.to_string();
            }
            let allow = params.profiles_allow();
            if !allow.is_empty() {
                cfg.allow = allow;
            }
        }
        let response = cogitator_lsp::initialize_response(req.id, crate::VERSION);
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
        feature: impl FnOnce(&DbtContext, Option<&cogitator_analysis::Document>, &str, Position) -> R,
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
                None => cogitator_lsp::Location {
                    uri: uri.to_string(),
                    range: cogitator_lsp::Range::zero_width(pos),
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
        // untyped arguments (01 §3.4); malformed → request dropped like Go
        let Some(arg) = params.arguments.first().and_then(|a| a.as_object()) else {
            return;
        };
        let uri = arg
            .get("uri")
            .and_then(|u| u.as_str())
            .unwrap_or_default()
            .to_string();
        match params.command.as_ref() {
            "dbt.columnLineage" => {
                self.column_lineage(req.id, &uri);
                return;
            }
            "dbt.openSchema" => {
                self.open_schema(req.id, &uri);
                return;
            }
            "dbt.showLineage" => {
                self.show_lineage(req.id, &uri);
                return;
            }
            "dbt.run" | "dbt.test" | "dbt.compile" => {
                let kind = params.command.trim_start_matches("dbt.");
                let kind: &'static str = match kind {
                    "run" => "run",
                    "test" => "test",
                    _ => "compile",
                };
                self.run_dbt(req.id, kind, &uri);
                return;
            }
            "dbt.extractModelYaml" => {
                self.extract_model_yaml(req.id, &uri);
                return;
            }
            "dbt.modelTests" => {
                let ctx = self.snapshot();
                let result = features::model_tests_json(&ctx, &uri);
                self.send(&ResultResponse::new(req.id, result));
                return;
            }
            "dbt.goToTests" => {
                self.go_to_tests(req.id, &uri);
                return;
            }
            "dbt.profiles" => {
                self.profiles_report(req.id);
                return;
            }
            "dbt.status" => {
                self.status_report(req.id);
                return;
            }
            "dbt.projectGraph" => {
                let ctx = self.snapshot();
                let result = features::project_graph_json(&ctx);
                self.send(&ResultResponse::new(req.id, result));
                return;
            }
            "dbt.setTarget" => {
                let target = arg
                    .get("target")
                    .and_then(|t| t.as_str())
                    .unwrap_or_default()
                    .to_string();
                let profile = arg
                    .get("profile")
                    .and_then(|p| p.as_str())
                    .map(str::to_string);
                self.set_target(req.id, target, profile);
                return;
            }
            "dbt.modelGraph" => {
                let model = arg.get("model").and_then(|m| m.as_str());
                let ctx = self.snapshot();
                let result = features::model_graph(&ctx, &uri, model);
                self.send(&ResultResponse::new(req.id, result));
                return;
            }
            "dbt.modelInfo" => {
                let model = arg.get("model").and_then(|m| m.as_str());
                let ctx = self.snapshot();
                let result = features::model_info(&ctx, &uri, model);
                self.send(&ResultResponse::new(req.id, result));
                return;
            }
            "dbt.relation" => {
                let ctx = self.snapshot();
                let cfg = self.profiles.lock().expect("profiles config lock").clone();
                let target = super::relation::active_target_relation(
                    &cfg,
                    &self.root_path,
                    &ctx.project.profile,
                );
                let result = features::relation_info(
                    &ctx,
                    self.documents.get(&uri),
                    &uri,
                    arg_position(arg),
                    target.as_ref(),
                );
                self.send(&ResultResponse::new(req.id, result));
                return;
            }
            "dbt.goToSchema" => {}
            other => {
                let response = ErrorResponse::new(
                    req.id,
                    error_codes::INVALID_PARAMS,
                    format!("unknown command: {other}"),
                );
                self.send(&response);
                return;
            }
        }
        let pos = arg_position(arg).unwrap_or_default();
        let ctx = self.snapshot();
        let result = features::go_to_schema(&ctx, self.documents.get(&uri), &uri, pos);
        self.send(&ResultResponse::new(req.id, result));
    }

    fn shutdown(&mut self, content: &[u8]) {
        let Ok(req) = serde_json::from_slice::<RequestId>(content) else {
            return;
        };
        tracing::info!("shutdown request");
        // Deliberate deviation from Go's bare response: JSON-RPC requires a
        // result member on success, and Neovim rejects the bare shape as
        // INVALID_SERVER_MESSAGE, leaving the client lingering on restart.
        self.send(&ResultResponse::new(req.id, serde_json::Value::Null));
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
                    .unwrap_or(cogitator_lsp::SemanticTokens { data: Vec::new() })
            },
        );
    }

    fn rename(&mut self, content: &[u8]) {
        let Ok(req) = serde_json::from_slice::<Envelope<cogitator_lsp::RenameParams>>(content)
        else {
            self.invalid_params(content, "textDocument/rename");
            return;
        };
        let Some(params) = req.params else {
            self.invalid_params(content, "textDocument/rename");
            return;
        };
        let ctx = self.snapshot();
        let uri = params.text_document.uri.into_owned();
        let edit = features::rename_model(
            &ctx,
            self.documents.get(&uri),
            &uri,
            params.position,
            &params.new_name,
        );
        self.send(&ResultResponse::new(req.id, edit));
    }

    fn code_lens(&mut self, content: &[u8]) {
        self.respond_position(content, "textDocument/codeLens", |ctx, _, uri, _| {
            features::code_lenses(ctx, uri)
        });
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
        let note = cogitator_lsp::publish_diagnostics(uri.to_string(), items);
        self.send(&note);
    }

    /// `dbt.columnLineage`: lineage for an open document, or for the file on
    /// disk when the editor has not opened it.
    fn column_lineage(&mut self, id: i64, uri: &str) {
        let ctx = self.snapshot();
        let result = match self.documents.get(uri) {
            Some(doc) => features_lineage(&doc.parsed, &ctx),
            None => {
                let path = cogitator_lsp::uri_to_fs_path(uri).unwrap_or_else(|| uri.to_string());
                std::fs::read_to_string(path).ok().and_then(|text| {
                    cogitator_parser::pipeline::parse_document(&text, &ctx.dialect)
                        .ok()
                        .and_then(|parsed| features_lineage(&parsed, &ctx))
                })
            }
        };
        self.send(&ResultResponse::new(id, result));
    }

    /// `dbt.openSchema` (lens entry point): resolve like goToSchema, then
    /// navigate the editor via window/showDocument.
    fn open_schema(&mut self, id: i64, uri: &str) {
        let ctx = self.snapshot();
        match features::go_to_schema(&ctx, self.documents.get(uri), uri, Position::default()) {
            Some(location) => {
                self.send_request(
                    "window/showDocument",
                    cogitator_lsp::ShowDocumentParams {
                        uri: location.uri,
                        take_focus: true,
                        selection: Some(location.range),
                    },
                );
            }
            None => self.show_message(
                cogitator_lsp::message_type::WARNING,
                "No schema entry found for this model.\n\
                 Reason: the model has no entry in a properties YAML file.\n\
                 Solution: add it under `models:` in a schema.yml inside your \
                 model-paths.",
            ),
        }
        self.send(&ResultResponse::new(id, serde_json::Value::Null));
    }

    /// `dbt.showLineage` (lens entry point): format the lineage into a
    /// window message instead of returning raw JSON.
    fn show_lineage(&mut self, id: i64, uri: &str) {
        let ctx = self.snapshot();
        let lineage = self.lineage_for(uri, &ctx);
        match lineage {
            Some(lineage) if !lineage.columns.is_empty() => {
                let mut lines: Vec<String> = lineage
                    .columns
                    .iter()
                    .take(20)
                    .map(|c| {
                        let sources: Vec<String> = c
                            .sources
                            .iter()
                            .map(|s| match s {
                                cogitator_analysis::lineage::SourceRef::Model { name, column } => {
                                    format!("{name}.{column}")
                                }
                                cogitator_analysis::lineage::SourceRef::Source {
                                    source,
                                    table,
                                    column,
                                } => format!("{source}.{table}.{column}"),
                                cogitator_analysis::lineage::SourceRef::Unknown {
                                    name,
                                    column,
                                } => format!("{name}.{column}"),
                            })
                            .collect();
                        format!("{} ← {}", c.name, sources.join(", "))
                    })
                    .collect();
                if lineage.columns.len() > 20 {
                    lines.push(format!("… and {} more columns", lineage.columns.len() - 20));
                }
                self.show_message(
                    cogitator_lsp::message_type::INFO,
                    format!("Column lineage:\n{}", lines.join("\n")),
                );
            }
            _ => self.show_message(
                cogitator_lsp::message_type::WARNING,
                "Could not derive column lineage for this model.\n\
                 Reason: the template contains Jinja control blocks \
                 ({% if %}/{% for %}) or the SQL did not parse as a single \
                 SELECT.\n\
                 Solution: lineage works on models whose rendered SQL is one \
                 SELECT statement; check for unclosed expressions or control \
                 flow.",
            ),
        }
        self.send(&ResultResponse::new(id, serde_json::Value::Null));
    }

    /// `dbt.run` / `dbt.test` / `dbt.compile` for one model, asynchronously.
    fn run_dbt(&mut self, id: i64, kind: &'static str, uri: &str) {
        let ctx = self.snapshot();
        let Some(root) = ctx.project_root.clone() else {
            self.show_message(
                cogitator_lsp::message_type::ERROR,
                format!(
                    "Cannot run dbt {kind}: no dbt project found.\n\
                     Reason: dbt_project.yml was not located from the \
                     workspace root '{}'.\n\
                     Solution: open the dbt project directory (the folder \
                     containing dbt_project.yml) or check the editor's \
                     rootUri.",
                    self.root_path
                ),
            );
            self.send(&ResultResponse::new(id, serde_json::Value::Null));
            return;
        };
        let binary = if self.fusion_path.is_empty() {
            "dbt".to_string()
        } else {
            self.fusion_path.clone()
        };
        let link = super::commands::ClientLink::new(
            self.write_tx.clone(),
            self.server_request_ids.clone(),
            self.progress_supported.clone(),
        );
        let target = {
            let cfg = self.profiles.lock().expect("profiles config lock");
            (!cfg.target.is_empty()).then(|| cfg.target.clone())
        };
        super::commands::spawn_dbt(
            link,
            super::commands::DbtInvocation {
                kind,
                binary,
                project_root: root,
                project_name: ctx.project.name.clone(),
                selector: super::fusion::model_selector_from_uri(uri),
                model_path: std::path::PathBuf::from(
                    cogitator_lsp::uri_to_fs_path(uri).unwrap_or_else(|| uri.to_string()),
                ),
                target,
            },
        );
        self.send(&ResultResponse::new(id, serde_json::Value::Null));
    }

    /// `dbt.extractModelYaml` (lens entry point): scaffold the schema YAML
    /// for an undocumented model, or add the next `versions:` entry to an
    /// existing one — applied in the editor via workspace/applyEdit.
    fn extract_model_yaml(&mut self, id: i64, uri: &str) {
        let ctx = self.snapshot();
        let lineage = self.lineage_for(uri, &ctx);
        match features::extract_model_yaml(&ctx, uri, lineage.as_ref()) {
            None => self.show_message(
                cogitator_lsp::message_type::WARNING,
                "Cannot extract a schema YAML here.\n\
                 Reason: this is not a known model, a <model>.yml already \
                 exists without an entry, or the schema file did not parse.\n\
                 Solution: open the model's .sql file and check its YAML.",
            ),
            Some(features::ExtractEdit::CreateFile { path, content }) => {
                let new_uri = cogitator_lsp::fs_path_to_uri(&path);
                self.send_request(
                    "workspace/applyEdit",
                    cogitator_lsp::ApplyWorkspaceEditParams {
                        label: "Extract model YAML".to_string(),
                        edit: cogitator_lsp::WorkspaceEdit {
                            document_changes: vec![
                                cogitator_lsp::DocumentChange::Create(
                                    cogitator_lsp::CreateFile::new(new_uri.clone()),
                                ),
                                cogitator_lsp::DocumentChange::Edit(
                                    cogitator_lsp::TextDocumentEdit::new(
                                        new_uri.clone(),
                                        vec![cogitator_lsp::TextEdit {
                                            range: cogitator_lsp::Range::default(),
                                            new_text: content,
                                        }],
                                    ),
                                ),
                            ],
                        },
                    },
                );
                self.send_request(
                    "window/showDocument",
                    cogitator_lsp::ShowDocumentParams {
                        uri: new_uri,
                        take_focus: true,
                        selection: None,
                    },
                );
                self.show_message(
                    cogitator_lsp::message_type::INFO,
                    format!(
                        "Created {} with the model's columns — fill in the \
                         descriptions and add data_tests.",
                        path.display()
                    ),
                );
            }
            Some(features::ExtractEdit::AddVersion {
                schema_path,
                edits,
                next,
            }) => {
                let schema_uri = cogitator_lsp::fs_path_to_uri(&schema_path);
                let selection = edits.first().map(|(range, _)| *range);
                let text_edits: Vec<cogitator_lsp::TextEdit> = edits
                    .iter()
                    .map(|(range, text)| cogitator_lsp::TextEdit {
                        range: *range,
                        new_text: text.clone(),
                    })
                    .collect();
                self.send_request(
                    "workspace/applyEdit",
                    cogitator_lsp::ApplyWorkspaceEditParams {
                        label: format!("Add model version {next}"),
                        edit: cogitator_lsp::WorkspaceEdit {
                            document_changes: vec![cogitator_lsp::DocumentChange::Edit(
                                cogitator_lsp::TextDocumentEdit::new(
                                    schema_uri.clone(),
                                    text_edits,
                                ),
                            )],
                        },
                    },
                );
                self.send_request(
                    "window/showDocument",
                    cogitator_lsp::ShowDocumentParams {
                        uri: schema_uri,
                        take_focus: true,
                        selection,
                    },
                );
                self.show_message(
                    cogitator_lsp::message_type::INFO,
                    format!(
                        "Added version {next} to {}.\n\
                         Next: create the versioned model file \
                         (<model>_v{next}.sql) or set `defined_in` on the \
                         version entry.",
                        schema_path.display()
                    ),
                );
            }
        }
        self.send(&ResultResponse::new(id, serde_json::Value::Null));
    }

    /// `dbt.goToTests` (lens entry point): jump to the first test defined
    /// for this model/version and list the rest.
    fn go_to_tests(&mut self, id: i64, uri: &str) {
        let ctx = self.snapshot();
        match features::model_tests(&ctx, uri) {
            Some((model, tests)) if !tests.is_empty() => {
                let first = &tests[0];
                self.send_request(
                    "window/showDocument",
                    cogitator_lsp::ShowDocumentParams {
                        uri: first.uri.clone(),
                        take_focus: true,
                        selection: Some(first.range),
                    },
                );
                if tests.len() > 1 {
                    let lines: Vec<String> = tests
                        .iter()
                        .map(|t| {
                            let mut line = format!("• {}", t.name);
                            if let Some(column) = &t.column {
                                line.push_str(&format!(" on {column}"));
                            }
                            if let Some(v) = t.version {
                                line.push_str(&format!(" (v{v})"));
                            }
                            line
                        })
                        .collect();
                    self.show_message(
                        cogitator_lsp::message_type::INFO,
                        format!("Tests for {model}:\n{}", lines.join("\n")),
                    );
                }
            }
            _ => self.show_message(
                cogitator_lsp::message_type::WARNING,
                "No tests defined for this model.\n\
                 Reason: its schema YAML has no data_tests/tests entries.\n\
                 Solution: add them under the model or its columns — \
                 dbt.extractModelYaml scaffolds the entry if it is missing.",
            ),
        }
        self.send(&ResultResponse::new(id, serde_json::Value::Null));
    }

    /// `dbt.status`: what the server knows and has been doing — project,
    /// dialect, active profile/target, backend, fusion, refresh stats.
    fn status_report(&mut self, id: i64) {
        let ctx = self.snapshot();
        let cfg = self.profiles.lock().expect("profiles config lock").clone();
        let info = self.status.lock().expect("status lock").clone();
        let fusion = if self.fusion_path.is_empty() {
            "off"
        } else if self.fusion_enabled.load(Ordering::Relaxed) {
            "on"
        } else {
            "configured (validating or invalid)"
        };
        let report = super::status::status_report(
            &info,
            &cfg,
            &ctx,
            fusion,
            self.progress_supported.load(Ordering::Relaxed),
        );
        self.send(&ResultResponse::new(id, report));
    }

    /// `dbt.profiles`: the redacted inventory within the allowed scope, the
    /// winning candidate file, and the active profile/target.
    fn profiles_report(&mut self, id: i64) {
        use cogitator_analysis::profiles::{load_inventory, ProfileScope};

        let ctx = self.snapshot();
        let cfg = self.profiles.lock().expect("profiles config lock").clone();
        let candidates =
            cogitator_analysis::util::profiles_candidates(&cfg.dir_override, &self.root_path);
        let searched: Vec<String> = candidates.iter().map(|p| p.display().to_string()).collect();
        let Some(path) = candidates.into_iter().find(|p| p.is_file()) else {
            self.show_message(
                cogitator_lsp::message_type::WARNING,
                format!(
                    "No profiles.yml found.\n\
                     Searched: {}.\n\
                     Solution: create one, or point --profiles-dir, \
                     initializationOptions.profilesDir or DBT_PROFILES_DIR at it.",
                    searched.join(", ")
                ),
            );
            self.send(&ResultResponse::new(
                id,
                cogitator_analysis::profiles::ProfilesReport::not_found(searched),
            ));
            return;
        };
        let scope = ProfileScope::Allowed {
            project: &ctx.project.profile,
            extra: &cfg.allow,
        };
        let inventory = load_inventory(&path, &scope);
        let active_profile = if cfg.profile.is_empty() {
            ctx.project.profile.clone()
        } else {
            cfg.profile.clone()
        };
        let active_target = if cfg.target.is_empty() {
            inventory
                .profile(&active_profile)
                .map(|p| p.default_target.clone())
                .unwrap_or_default()
        } else {
            cfg.target.clone()
        };
        let mut report = inventory.report();
        report.searched = searched;
        report.active_profile = Some(active_profile);
        report.active_target = Some(active_target);
        self.send(&ResultResponse::new(id, report));
    }

    /// `dbt.setTarget`: switch the active profile/target (allowlist-checked),
    /// trigger a context refresh, and confirm with the target's auth status.
    fn set_target(&mut self, id: i64, target: String, profile: Option<String>) {
        use cogitator_analysis::profiles::{load_inventory, ProfileScope};

        let respond_null = |me: &mut Self| {
            me.send(&ResultResponse::new(id, serde_json::Value::Null));
        };
        if target.is_empty() {
            self.show_message(
                cogitator_lsp::message_type::ERROR,
                "dbt.setTarget needs a target name.\n\
                 Solution: pass arguments: [{ \"target\": \"dev\" }], optionally \
                 with \"profile\".",
            );
            respond_null(self);
            return;
        }
        let ctx = self.snapshot();
        let cfg = self.profiles.lock().expect("profiles config lock").clone();
        let Some(path) =
            cogitator_analysis::util::profiles_candidates(&cfg.dir_override, &self.root_path)
                .into_iter()
                .find(|p| p.is_file())
        else {
            self.show_message(
                cogitator_lsp::message_type::ERROR,
                "Cannot switch target: no profiles.yml found.\n\
                 Solution: create one, or point --profiles-dir / \
                 initializationOptions.profilesDir / DBT_PROFILES_DIR at it.",
            );
            respond_null(self);
            return;
        };
        let scope = ProfileScope::Allowed {
            project: &ctx.project.profile,
            extra: &cfg.allow,
        };
        let inventory = load_inventory(&path, &scope);
        let profile_given = profile.as_deref().is_some_and(|p| !p.is_empty());
        let profile_name = profile.filter(|p| !p.is_empty()).unwrap_or_else(|| {
            if cfg.profile.is_empty() {
                ctx.project.profile.clone()
            } else {
                cfg.profile.clone()
            }
        });
        let Some(info) = inventory.profile(&profile_name) else {
            self.show_message(
                cogitator_lsp::message_type::ERROR,
                format!(
                    "Profile '{profile_name}' not found in {}.",
                    inventory.path.display()
                ),
            );
            respond_null(self);
            return;
        };
        if !info.allowed {
            self.show_message(
                cogitator_lsp::message_type::ERROR,
                format!(
                    "Profile '{profile_name}' is not readable.\n\
                     Reason: by default only the project's own profile is; \
                     '{profile_name}' is not on the allowlist.\n\
                     Solution: add it via --profiles-allow {profile_name} or \
                     initializationOptions.profilesAllow."
                ),
            );
            respond_null(self);
            return;
        }
        let Some(chosen) = info.targets.iter().find(|t| t.name == target) else {
            let available: Vec<&str> = info.targets.iter().map(|t| t.name.as_str()).collect();
            self.show_message(
                cogitator_lsp::message_type::ERROR,
                format!(
                    "Target '{target}' not found in profile '{profile_name}'.\n\
                     Available targets: {}.",
                    available.join(", ")
                ),
            );
            respond_null(self);
            return;
        };
        let mut message = format!(
            "Active target: {profile_name}/{target} ({}, auth: {}).",
            chosen.adapter_type,
            chosen.auth.method.as_str()
        );
        let level = if chosen.auth.issues.is_empty() {
            cogitator_lsp::message_type::INFO
        } else {
            message.push_str("\nAuth problems:");
            for issue in &chosen.auth.issues {
                message.push_str("\n• ");
                message.push_str(&issue.message);
            }
            cogitator_lsp::message_type::WARNING
        };
        {
            let mut live = self.profiles.lock().expect("profiles config lock");
            live.target = target.clone();
            if profile_given {
                live.profile = profile_name.clone();
            }
        }
        let _ = self.refresh_tx.send(self.root_path.clone());
        self.show_message(level, message);
        self.send(&ResultResponse::new(
            id,
            super::status::SetTargetResult {
                profile: profile_name,
                target,
            },
        ));
    }

    fn lineage_for(
        &self,
        uri: &str,
        ctx: &DbtContext,
    ) -> Option<cogitator_analysis::lineage::ModelLineage> {
        match self.documents.get(uri) {
            Some(doc) => cogitator_analysis::lineage::column_lineage(&doc.parsed, &ctx.dialect),
            None => {
                let path = cogitator_lsp::uri_to_fs_path(uri).unwrap_or_else(|| uri.to_string());
                let text = std::fs::read_to_string(path).ok()?;
                let parsed =
                    cogitator_parser::pipeline::parse_document(&text, &ctx.dialect).ok()?;
                cogitator_analysis::lineage::column_lineage(&parsed, &ctx.dialect)
            }
        }
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

fn features_lineage(
    parsed: &cogitator_parser::ParsedDocument,
    ctx: &DbtContext,
) -> Option<cogitator_analysis::lineage::ModelLineage> {
    cogitator_analysis::lineage::column_lineage(parsed, &ctx.dialect)
}

/// The optional `position` member of a command argument object.
fn arg_position(arg: &serde_json::Map<String, serde_json::Value>) -> Option<Position> {
    arg.get("position").map(|p| Position {
        line: p.get("line").and_then(|v| v.as_f64()).unwrap_or(0.0) as u32,
        character: p.get("character").and_then(|v| v.as_f64()).unwrap_or(0.0) as u32,
    })
}
