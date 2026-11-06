//! Method dispatch (specs/rust-port/01 §3). One span per message (05 §6.3);
//! unknown methods ignored; parse failures logged and dropped, never an
//! error response — faithful to Go.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use cogitator_analysis::{DbtContext, Documents};

use cogitator_lsp::{
    error_codes, rpc, Envelope, ErrorResponse, InitializeParams, MaybeId, Position, RequestId,
    ResultResponse,
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
    /// Hands the config watcher its root at initialize.
    config_root_tx: watch::Sender<Option<String>>,
    /// Fast-path reload trigger (didSave/didOpen of the file, dbt.reloadConfig).
    config_poke_tx: mpsc::UnboundedSender<()>,
    /// The watcher's view, reported by dbt.status.
    config_status: Arc<std::sync::Mutex<super::config_watcher::ConfigStatus>>,
    /// Hot lint/warehouse switches (cogitator.yml).
    toggles: Arc<super::SharedToggles>,
    /// Running dbt invocations, cancellable by token (dbt.cancel).
    jobs: Arc<super::jobs::DbtJobs>,
    /// Bumped per context rebuild — the workspace/diagnostic resultId.
    context_revision: Arc<std::sync::atomic::AtomicU64>,
}

mod commands_exec;
mod dbt_ops;
mod diagnostics_req;
mod requests;
mod sync;

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
        config_root_tx: watch::Sender<Option<String>>,
        config_poke_tx: mpsc::UnboundedSender<()>,
        config_status: Arc<std::sync::Mutex<super::config_watcher::ConfigStatus>>,
        toggles: Arc<super::SharedToggles>,
        jobs: Arc<super::jobs::DbtJobs>,
        context_revision: Arc<std::sync::atomic::AtomicU64>,
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
            config_root_tx,
            config_poke_tx,
            config_status,
            toggles,
            jobs,
            context_revision,
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
            "textDocument/prepareRename" => self.prepare_rename(content),
            "textDocument/prepareCallHierarchy" => self.prepare_call_hierarchy(content),
            "callHierarchy/incomingCalls" => self.call_hierarchy_incoming(content),
            // outgoing calls: deliberately empty — the macro hierarchy is
            // one level deep today, and clients probe both directions
            "callHierarchy/outgoingCalls" => self.call_hierarchy_outgoing(content),
            "textDocument/codeLens" => self.code_lens(content),
            "textDocument/codeAction" => self.code_action(content),
            "textDocument/inlayHint" => self.inlay_hint(content),
            "textDocument/documentLink" => self.document_link(content),
            "textDocument/linkedEditingRange" => self.linked_editing(content),
            "textDocument/formatting" => self.formatting(content),
            "textDocument/diagnostic" => self.pull_diagnostics(content),
            "workspace/diagnostic" => self.workspace_diagnostics(content),
            "workspace/symbol" => self.workspace_symbol(content),
            "workspace/didChangeWatchedFiles" => {
                // best-effort: clients only send this after dynamic
                // registration, which this server does not request yet
                let _ = self.refresh_tx.send(self.root_path.clone());
            }
            "$/setTrace" => {
                #[derive(serde::Deserialize)]
                struct SetTraceParams {
                    value: String,
                }
                if let Ok(req) = serde_json::from_slice::<Envelope<SetTraceParams>>(content) {
                    if let Some(wanted) = req
                        .params
                        .as_ref()
                        .and_then(|p| crate::client_log::level_from_trace_value(&p.value))
                    {
                        crate::client_log::set_level(wanted);
                        tracing::debug!(value = %req.params.unwrap().value, "trace level set");
                    }
                }
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
        let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
        tracing::debug!(target: "cogitator::requests", %method, elapsed_ms, "handled");
        crate::metrics::record_request(&method, elapsed_ms);
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
        if let Some(wanted) = params
            .trace
            .as_deref()
            .and_then(crate::client_log::level_from_trace_value)
        {
            crate::client_log::set_level(wanted);
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
        // the config watcher waits on the root; its first pass is immediate
        let _ = self.config_root_tx.send(Some(self.root_path.clone()));
    }

    /// Is this URI the workspace's cogitator.yml? Compared as `Path`
    /// components (08 §1: never as strings — drive letters, separators).
    fn is_config_uri(&self, uri: &str) -> bool {
        if self.root_path.is_empty() {
            return false;
        }
        let Some(path) = cogitator_lsp::uri_to_fs_path(uri) else {
            return false;
        };
        let path = std::path::Path::new(&path);
        path.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| super::config_watcher::FILE_NAMES.contains(&n))
            && path.parent() == Some(std::path::Path::new(&self.root_path))
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
}

fn arg_position(arg: &serde_json::Map<String, serde_json::Value>) -> Option<Position> {
    arg.get("position").map(|p| Position {
        line: p.get("line").and_then(|v| v.as_f64()).unwrap_or(0.0) as u32,
        character: p.get("character").and_then(|v| v.as_f64()).unwrap_or(0.0) as u32,
    })
}
