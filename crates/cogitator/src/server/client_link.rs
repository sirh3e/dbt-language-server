//! THE single send boundary: every frame — responses, notifications,
//! server->client requests, diagnostics, progress — leaves through one
//! cloneable `ClientLink`, so the dispatcher task and offloaded tasks
//! share identical plumbing (and the one request-id counter).

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use cogitator_lsp::{rpc, Diagnostic, Range, ResultResponse, ServerRequest, ShowDocumentParams};
use serde::Serialize;
use tokio::sync::mpsc;

/// Writer-channel handle for background tasks (the dispatcher owns its own).
#[derive(Clone)]
pub struct ClientLink {
    write_tx: mpsc::UnboundedSender<Vec<u8>>,
    ids: Arc<AtomicI64>,
    /// Client advertised window.workDoneProgress (set at initialize).
    progress_supported: Arc<std::sync::atomic::AtomicBool>,
}

impl ClientLink {
    /// Serialize and ship one frame. Each call owns its buffer — frames
    /// are built once per message, the owner boundary this allocation
    /// feeds (08 §1.5); a shared scratch would force &mut everywhere.
    pub fn send<T: Serialize>(&self, msg: &T) {
        if let Ok(frame) = rpc::encode_frame_vec(msg) {
            let _ = self.write_tx.send(frame);
        }
    }

    /// Answer a request. (Consumed by the offload shell; allowed dead
    /// until that commit lands in this same series.)
    #[allow(dead_code)]
    pub fn respond<T: Serialize>(&self, id: i64, result: T) {
        self.send(&ResultResponse::new(id, result));
    }

    /// Answer a request with a JSON-RPC error object.
    #[allow(dead_code)]
    pub fn respond_error(&self, id: i64, code: i32, message: String) {
        self.send(&cogitator_lsp::ErrorResponse::new(id, code, message));
    }

    /// Send a server→client request (ids from the ONE shared counter).
    pub fn send_request<T: Serialize>(&self, method: &'static str, params: T) {
        let id = self.ids.fetch_add(1, Ordering::Relaxed);
        self.send(&ServerRequest {
            jsonrpc: "2.0",
            id,
            method,
            params,
        });
    }

    #[allow(dead_code)]
    pub fn publish_diagnostics(&self, uri: String, items: Vec<Diagnostic>) {
        self.send(&cogitator_lsp::publish_diagnostics(uri, items));
    }

    /// The raw writer channel (fusion jobs stream frames themselves).
    pub fn writer(&self) -> mpsc::UnboundedSender<Vec<u8>> {
        self.write_tx.clone()
    }
}

/// One $/progress series; falls back to silence when the client did not
/// advertise window.workDoneProgress (callers keep their showMessage
/// fallbacks for the outcomes that matter).
pub struct Progress {
    link: Option<ClientLink>,
    token: String,
}

impl Progress {
    pub fn active(&self) -> bool {
        self.link.is_some()
    }

    pub fn end(self, message: impl Into<String>) {
        if let Some(link) = &self.link {
            link.progress_value(
                &self.token,
                cogitator_lsp::ProgressValue::End {
                    message: message.into(),
                },
            );
        }
    }
}

impl ClientLink {
    pub fn new(
        write_tx: mpsc::UnboundedSender<Vec<u8>>,
        ids: Arc<AtomicI64>,
        progress_supported: Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        ClientLink {
            write_tx,
            ids,
            progress_supported,
        }
    }

    /// Start a work-done progress series ($/progress): token create request
    /// plus the begin notification. A no-op handle when unsupported.
    pub fn progress_begin(&self, title: impl Into<String>) -> Progress {
        if !self.progress_supported.load(Ordering::Relaxed) {
            return Progress {
                link: None,
                token: String::new(),
            };
        }
        let token = format!("cogitator/{}", self.ids.fetch_add(1, Ordering::Relaxed));
        let id = self.ids.fetch_add(1, Ordering::Relaxed);
        self.send(&ServerRequest {
            jsonrpc: "2.0",
            id,
            method: "window/workDoneProgress/create",
            params: cogitator_lsp::WorkDoneProgressCreateParams {
                token: token.clone(),
            },
        });
        self.progress_value(
            &token,
            cogitator_lsp::ProgressValue::Begin {
                title: title.into(),
            },
        );
        Progress {
            link: Some(self.clone()),
            token,
        }
    }

    fn progress_value(&self, token: &str, value: cogitator_lsp::ProgressValue) {
        self.send(&cogitator_lsp::Notification {
            jsonrpc: "2.0",
            method: "$/progress",
            params: cogitator_lsp::ProgressParams {
                token: token.to_string(),
                value,
            },
        });
    }

    pub fn show_message(&self, message_type: i32, message: impl Into<String>) {
        let note = cogitator_lsp::show_message(message_type, message);
        self.send(&note);
    }

    pub fn show_document(&self, uri: String, selection: Option<Range>) {
        let id = self.ids.fetch_add(1, Ordering::Relaxed);
        let request = ServerRequest {
            jsonrpc: "2.0",
            id,
            method: "window/showDocument",
            params: ShowDocumentParams {
                uri,
                take_focus: true,
                selection,
            },
        };
        self.send(&request);
    }
}
