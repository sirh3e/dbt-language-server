//! Writer-channel handle for background tasks: showMessage/showDocument
//! server->client traffic and the $/progress series ($/progress begin/end,
//! with token create requests), gated on the client's advertised
//! window.workDoneProgress capability.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use cogitator_lsp::{rpc, Range, ServerRequest, ShowDocumentParams};
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

    fn send<T: Serialize>(&self, msg: &T) {
        if let Ok(frame) = rpc::encode_frame_vec(msg) {
            let _ = self.write_tx.send(frame);
        }
    }
}
