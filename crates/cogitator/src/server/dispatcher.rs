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
    /// THE send boundary (clone it for offloaded/background work).
    link: super::client_link::ClientLink,
    ctx_rx: watch::Receiver<Arc<DbtContext>>,
    refresh_tx: mpsc::UnboundedSender<String>,
    root_path: String,
    fusion_path: String,
    fusion_enabled: Arc<AtomicBool>,
    fusion_jobs: HashMap<String, AbortHandle>,
    /// Shared with the refresh task: initializationOptions and
    /// dbt.setTarget replace the launch-flag values here.
    profiles: Arc<std::sync::Mutex<super::ProfilesConfig>>,
    /// Client advertised window.workDoneProgress (set at initialize).
    progress_supported: Arc<AtomicBool>,
    /// What the server has been doing, for dbt.status.
    status: Arc<std::sync::Mutex<super::StatusInfo>>,
    /// Shared derived-data caches (profiles inventory, lineage, results).
    derived: Arc<super::derived_cache::DerivedCache>,
    /// Offloaded requests by id — real $/cancelRequest lives here.
    in_flight: super::offload::InFlight,
    /// Hot-tunable numeric settings (cogitator.yml).
    limits: Arc<super::SharedLimits>,
}

impl Dispatcher {
    // constructed exactly once, in server::run — the shared handles are
    // clearer spelled out than bundled into a one-off struct
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        documents: Documents,
        link: super::client_link::ClientLink,
        ctx_rx: watch::Receiver<Arc<DbtContext>>,
        refresh_tx: mpsc::UnboundedSender<String>,
        fusion_path: String,
        fusion_enabled: Arc<AtomicBool>,
        profiles: Arc<std::sync::Mutex<super::ProfilesConfig>>,
        progress_supported: Arc<AtomicBool>,
        status: Arc<std::sync::Mutex<super::StatusInfo>>,
        derived: Arc<super::derived_cache::DerivedCache>,
        limits: Arc<super::SharedLimits>,
    ) -> Self {
        Dispatcher {
            documents,
            link,
            ctx_rx,
            refresh_tx,
            root_path: String::new(),
            fusion_path,
            fusion_enabled,
            fusion_jobs: HashMap::new(),
            profiles,
            progress_supported,
            status,
            derived,
            in_flight: super::offload::InFlight::default(),
            limits,
        }
    }

    fn snapshot(&self) -> Arc<DbtContext> {
        self.ctx_rx.borrow().clone()
    }

    /// The per-message snapshot handed to pure handlers (and, from the
    /// offload commit, to spawned tasks): Arc clones + a tight cfg lock.
    fn snapshot_for(&self, uri: &str) -> super::snapshot::Snapshot {
        super::snapshot::Snapshot {
            ctx: self.snapshot(),
            doc: (!uri.is_empty())
                .then(|| self.documents.get_arc(uri))
                .flatten(),
            root_path: self.root_path.clone(),
            profiles: self.profiles.lock().expect("profiles config lock").clone(),
            fusion_path: self.fusion_path.clone(),
            fusion_enabled: self.fusion_enabled.load(Ordering::Relaxed),
            compiled_cap_bytes: self.limits.compiled_cap_bytes.load(Ordering::Relaxed),
        }
    }

    /// Ship a handler's reply: effects in order, response last.
    fn transmit(&self, id: Option<i64>, reply: super::reply::Reply) {
        super::reply::transmit(&self.link, &self.refresh_tx, &self.root_path, id, reply);
    }

    /// Run a slow/fs handler off this task (05 §6.1: the dispatcher task
    /// never touches the filesystem). Decode + snapshot happen BEFORE
    /// this call, on the dispatcher task, so didChange ordering holds.
    fn offload_blocking(
        &mut self,
        id: i64,
        name: &'static str,
        job: impl FnOnce() -> super::reply::Reply + Send + 'static,
    ) {
        let out = super::offload::OffloadLink {
            link: self.link.clone(),
            refresh_tx: self.refresh_tx.clone(),
            root_path: self.root_path.clone(),
        };
        self.in_flight.spawn_blocking(id, name, out, job);
    }

    fn show_message(&mut self, message_type: i32, message: impl Into<String>) {
        self.link.show_message(message_type, message);
    }

    fn send<T: Serialize>(&mut self, msg: &T) {
        self.link.send(msg);
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
        // INFO so OTLP sees it under default filters (05 §6.3: span
        // duration is the latency measurement); request id when present.
        let id = serde_json::from_slice::<MaybeId>(content)
            .ok()
            .and_then(|m| m.id);
        let span = tracing::info_span!("lsp_message", %method, id = tracing::field::Empty);
        if let Some(id) = id {
            span.record("id", id);
        }
        let _enter = span.enter();
        let started = std::time::Instant::now();

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
            "textDocument/codeAction" => self.code_action(content),
            "textDocument/diagnostic" => self.pull_diagnostics(content),
            "workspace/symbol" => self.workspace_symbol(content),
            "workspace/didChangeWatchedFiles" => {
                // best-effort: clients only send this after dynamic
                // registration, which this server does not request yet
                let _ = self.refresh_tx.send(self.root_path.clone());
            }
            "$/cancelRequest" => {
                #[derive(serde::Deserialize)]
                struct CancelParams {
                    id: i64,
                }
                if let Ok(req) = serde_json::from_slice::<Envelope<CancelParams>>(content) {
                    if let Some(params) = req.params {
                        self.in_flight.cancel(params.id, &self.link);
                    }
                }
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
        crate::metrics::record_request(&method, started.elapsed().as_secs_f64() * 1000.0);
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
            self.invalid_params(content, "initialize");
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
            self.invalid_params(content, "workspace/executeCommand");
            return;
        };
        let Some(params) = req.params else {
            self.invalid_params(content, "workspace/executeCommand");
            return;
        };
        // a malformed argument used to hang the client on a silent drop
        let Some(arg) = params.arguments.first().and_then(|a| a.as_object()) else {
            let guidance = cogitator_error::guidance::Guidance::new(format!(
                "'{}' got no usable argument.",
                params.command
            ))
            .because("every dbt.* command takes exactly one argument OBJECT.")
            .fix("send arguments: [{\"uri\": \"file://…\", …}] (spec §1.6).");
            self.send(&ErrorResponse::new(
                req.id,
                error_codes::INVALID_PARAMS,
                guidance.to_message(),
            ));
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
            "dbt.run" | "dbt.test" | "dbt.compile" | "dbt.parse" => {
                let kind = params.command.trim_start_matches("dbt.");
                let kind: &'static str = match kind {
                    "run" => "run",
                    "test" => "test",
                    "parse" => "parse",
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
                let snap = self.snapshot_for(&uri);
                let derived = self.derived.clone();
                self.offload_blocking(req.id, "dbt.modelTests", move || {
                    super::handlers::commands::model_tests(&snap, &derived, &uri)
                });
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
                self.offload_blocking(req.id, "dbt.projectGraph", move || {
                    super::reply::Reply::result(&features::project_graph_json(&ctx))
                });
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
            "dbt.cache" => {
                let action = arg
                    .get("action")
                    .and_then(|a| a.as_str())
                    .unwrap_or("stats");
                let reply = super::handlers::commands::cache(&self.derived, action);
                self.transmit(Some(req.id), reply);
                return;
            }
            "dbt.setProfilesDir" => {
                let dir = arg
                    .get("dir")
                    .and_then(|d| d.as_str())
                    .unwrap_or_default()
                    .to_string();
                self.set_profiles_dir(req.id, dir);
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
                let model = arg
                    .get("model")
                    .and_then(|m| m.as_str())
                    .map(str::to_string);
                let snap = self.snapshot_for(&uri);
                let derived = self.derived.clone();
                self.offload_blocking(req.id, "dbt.modelInfo", move || {
                    super::handlers::commands::model_info(&snap, &derived, &uri, model.as_deref())
                });
                return;
            }
            "dbt.openModel" => {
                let model = arg.get("model").and_then(|m| m.as_str());
                let snap = self.snapshot_for(&uri);
                let reply = super::handlers::navigation::open_model(&snap, &uri, model);
                self.transmit(Some(req.id), reply);
                return;
            }
            "dbt.compiledSql" => {
                let model = arg
                    .get("model")
                    .and_then(|m| m.as_str())
                    .map(str::to_string);
                let snap = self.snapshot_for(&uri);
                self.offload_blocking(req.id, "dbt.compiledSql", move || {
                    super::handlers::commands::compiled_sql(&snap, &uri, model.as_deref())
                });
                return;
            }
            "dbt.relation" => {
                let pos = arg_position(arg);
                let snap = self.snapshot_for(&uri);
                let derived = self.derived.clone();
                self.offload_blocking(req.id, "dbt.relation", move || {
                    super::handlers::profiles::relation(&snap, &derived, &uri, pos)
                });
                return;
            }
            "dbt.goToSchema" => {}
            other => {
                let guidance = cogitator_error::guidance::Guidance::new(format!(
                    "Unknown command '{other}'."
                ))
                .suggest(cogitator_error::guidance::closest_matches(
                    other,
                    cogitator_lsp::COMMANDS.iter().copied(),
                ))
                .fix("the full roster is in the initialize response's executeCommandProvider.");
                let response =
                    ErrorResponse::new(req.id, error_codes::INVALID_PARAMS, guidance.to_message());
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
            self.invalid_params(content, "shutdown");
            return;
        };
        tracing::info!("shutdown request");
        self.in_flight.abort_all();
        // Deliberate deviation from Go's bare response: JSON-RPC requires a
        // result member on success, and Neovim rejects the bare shape as
        // INVALID_SERVER_MESSAGE, leaving the client lingering on restart.
        self.send(&ResultResponse::new(req.id, serde_json::Value::Null));
    }

    fn resolve_completion(&mut self, content: &[u8]) {
        let Ok(req) = serde_json::from_slice::<MaybeId>(content) else {
            self.invalid_params(content, "completionItem/resolve");
            return;
        };
        let Some(id) = req.id else { return };
        let Ok(item) = serde_json::from_slice::<serde_json::Value>(content) else {
            self.invalid_params(content, "completionItem/resolve");
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
        let uri = params.text_document.uri.into_owned();
        let new_name = params.new_name.into_owned();
        let position = params.position;
        let snap = self.snapshot_for(&uri);
        self.offload_blocking(req.id, "textDocument/rename", move || {
            let edit =
                features::rename_model(&snap.ctx, snap.doc.as_deref(), &uri, position, &new_name);
            super::reply::Reply::result(&edit)
        });
    }

    fn code_lens(&mut self, content: &[u8]) {
        self.respond_position(content, "textDocument/codeLens", |ctx, _, uri, _| {
            features::code_lenses(ctx, uri)
        });
    }

    fn code_action(&mut self, content: &[u8]) {
        let Ok(req) = serde_json::from_slice::<Envelope<cogitator_lsp::CodeActionParams>>(content)
        else {
            self.invalid_params(content, "textDocument/codeAction");
            return;
        };
        let Some(params) = req.params else {
            self.invalid_params(content, "textDocument/codeAction");
            return;
        };
        let uri = params.text_document.uri.into_owned();
        let only: Option<Vec<String>> = params
            .context
            .and_then(|c| c.only)
            .map(|kinds| kinds.into_iter().map(|k| k.into_owned()).collect());
        let snap = self.snapshot_for(&uri);
        // profiles.yml buffers offer "Switch target" actions instead
        let targets = super::handlers::profiles::target_choices(&snap, &self.derived, &uri);
        let actions = features::code_actions(
            &snap.ctx,
            snap.doc.as_deref(),
            &uri,
            params.range,
            only.as_deref(),
            targets.as_deref(),
        );
        self.send(&ResultResponse::new(req.id, actions));
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
        let snap = self.snapshot_for(&uri);
        self.offload_blocking(id, "textDocument/diagnostic", move || {
            let items =
                super::diagnostics::static_diagnostics(&snap.ctx, snap.doc.as_deref(), &uri);
            super::reply::Reply::result(&FullDocumentDiagnosticReport {
                kind: "full",
                items,
            })
        });
    }

    /// Diagnostics for one open doc — see server::diagnostics.
    fn static_diagnostics(&self, uri: &str) -> Vec<Diagnostic> {
        super::diagnostics::static_diagnostics(&self.snapshot(), self.documents.get(uri), uri)
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

    fn column_lineage(&mut self, id: i64, uri: &str) {
        let snap = self.snapshot_for(uri);
        let derived = self.derived.clone();
        let uri = uri.to_string();
        self.offload_blocking(id, "dbt.columnLineage", move || {
            let lineage = super::handlers::commands::lineage_for(
                &derived,
                &snap.ctx,
                snap.doc.as_deref(),
                &uri,
            );
            super::handlers::commands::column_lineage(&lineage)
        });
    }

    fn open_schema(&mut self, id: i64, uri: &str) {
        let snap = self.snapshot_for(uri);
        let reply = super::handlers::navigation::open_schema(&snap, uri);
        self.transmit(Some(id), reply);
    }

    fn show_lineage(&mut self, id: i64, uri: &str) {
        let snap = self.snapshot_for(uri);
        let derived = self.derived.clone();
        let uri = uri.to_string();
        self.offload_blocking(id, "dbt.showLineage", move || {
            let lineage = super::handlers::commands::lineage_for(
                &derived,
                &snap.ctx,
                snap.doc.as_deref(),
                &uri,
            );
            super::handlers::navigation::show_lineage((*lineage).as_ref())
        });
    }

    /// `dbt.run` / `dbt.test` / `dbt.compile` for one model, asynchronously.
    fn run_dbt(&mut self, id: i64, kind: &'static str, uri: &str) {
        let ctx = self.snapshot();
        let Some(root) = ctx.project_root.clone() else {
            self.show_message(
                cogitator_lsp::message_type::ERROR,
                cogitator_error::guidance::Guidance::new(format!(
                    "Cannot run dbt {kind}: no dbt project found."
                ))
                .because(format!(
                    "dbt_project.yml was not located from the workspace root '{}'.",
                    self.root_path
                ))
                .fix(
                    "open the dbt project directory (the folder containing \
                     dbt_project.yml) or check the editor's rootUri.",
                )
                .to_message(),
            );
            self.send(&ResultResponse::new(id, serde_json::Value::Null));
            return;
        };
        let binary = if self.fusion_path.is_empty() {
            "dbt".to_string()
        } else {
            self.fusion_path.clone()
        };
        let link = self.link.clone();
        let (target, profiles_dir, profile) = {
            let cfg = self.profiles.lock().expect("profiles config lock");
            (
                (!cfg.target.is_empty()).then(|| cfg.target.clone()),
                (!cfg.dir_override.is_empty()).then(|| cfg.dir_override.clone()),
                (!cfg.profile.is_empty()).then(|| cfg.profile.clone()),
            )
        };
        // parse is project-wide (no --select) and the only kind whose
        // success refreshes the context: its whole point is fresh artifacts
        let selector = (kind != "parse").then(|| super::fusion::model_selector_from_uri(uri));
        let refresh = (kind == "parse").then(|| (self.refresh_tx.clone(), self.root_path.clone()));
        super::commands::spawn_dbt(
            link,
            super::commands::DbtInvocation {
                kind,
                binary,
                project_root: root,
                project_name: ctx.project.name.clone(),
                selector,
                model_path: std::path::PathBuf::from(
                    cogitator_lsp::uri_to_fs_path(uri).unwrap_or_else(|| uri.to_string()),
                ),
                target,
                profiles_dir,
                profile,
                refresh,
            },
        );
        self.send(&ResultResponse::new(id, serde_json::Value::Null));
    }

    fn extract_model_yaml(&mut self, id: i64, uri: &str) {
        let snap = self.snapshot_for(uri);
        let derived = self.derived.clone();
        let uri = uri.to_string();
        self.offload_blocking(id, "dbt.extractModelYaml", move || {
            let lineage = super::handlers::commands::lineage_for(
                &derived,
                &snap.ctx,
                snap.doc.as_deref(),
                &uri,
            );
            super::handlers::navigation::extract_model_yaml(&snap, &uri, (*lineage).as_ref())
        });
    }

    fn go_to_tests(&mut self, id: i64, uri: &str) {
        let snap = self.snapshot_for(uri);
        let uri = uri.to_string();
        self.offload_blocking(id, "dbt.goToTests", move || {
            super::handlers::navigation::go_to_tests(&snap, &uri)
        });
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
            self.derived.report(),
        );
        self.send(&ResultResponse::new(id, report));
    }

    fn profiles_report(&mut self, id: i64) {
        let snap = self.snapshot_for("");
        let derived = self.derived.clone();
        self.offload_blocking(id, "dbt.profiles", move || {
            super::handlers::profiles::report(&snap, &derived)
        });
    }

    fn set_profiles_dir(&mut self, id: i64, dir: String) {
        let live_cfg = self.profiles.clone();
        self.offload_blocking(id, "dbt.setProfilesDir", move || {
            let outcome = super::handlers::profiles::set_profiles_dir(dir);
            if let Some(dir) = outcome.apply {
                let mut cfg = live_cfg.lock().expect("profiles config lock");
                cfg.dir_override = dir;
                cfg.profile.clear();
                cfg.target.clear();
            }
            outcome.reply
        });
    }

    fn set_target(&mut self, id: i64, target: String, profile: Option<String>) {
        let snap = self.snapshot_for("");
        let derived = self.derived.clone();
        let live_cfg = self.profiles.clone();
        self.offload_blocking(id, "dbt.setTarget", move || {
            let outcome = super::handlers::profiles::set_target(&snap, &derived, target, profile);
            if let Some((profile_override, target)) = outcome.apply {
                // tight std lock, no await in scope
                let mut live = live_cfg.lock().expect("profiles config lock");
                live.target = target;
                if let Some(profile) = profile_override {
                    live.profile = profile;
                }
            }
            outcome.reply
        });
    }

    /// Fusion compile per 04 §2, async + cancellable per URI (fixes Q9).
    fn spawn_fusion(&mut self, uri: String) {
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

/// The optional `position` member of a command argument object.
fn arg_position(arg: &serde_json::Map<String, serde_json::Value>) -> Option<Position> {
    arg.get("position").map(|p| Position {
        line: p.get("line").and_then(|v| v.as_f64()).unwrap_or(0.0) as u32,
        character: p.get("character").and_then(|v| v.as_f64()).unwrap_or(0.0) as u32,
    })
}
