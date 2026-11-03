//! The Elm-style handler contract (05 §6.2): a handler is a pure
//! function producing a `Reply` — ordered side effects first, the
//! response last — and the shell only transmits. The e2e suite's stash
//! assertions depend on exactly that order (effects reach the wire
//! before the response), so `transmit` is the one place it lives.

use cogitator_lsp::Diagnostic;
use serde::Serialize;

use super::client_link::ClientLink;

/// One ordered side effect of handling a message.
pub enum Effect {
    ShowMessage {
        level: i32,
        text: String,
    },
    ShowDocument(cogitator_lsp::ShowDocumentParams),
    ApplyEdit(cogitator_lsp::ApplyWorkspaceEditParams),
    #[allow(dead_code)] // the offload shell publishes through effects
    PublishDiagnostics {
        uri: String,
        items: Vec<Diagnostic>,
    },
    /// Trigger a context refresh (the shell owns the channel).
    Refresh,
}

/// The response member. `Raw` carries pre-serialized JSON (RawValue —
/// `serde_json::Value` would alphabetize keys and change the wire).
pub enum Response {
    Raw(Box<serde_json::value::RawValue>),
    /// Consumed from the cancellation/invalid-params commits onward.
    #[allow(dead_code)]
    Error {
        code: i32,
        message: String,
    },
    /// Notifications, or requests already answered elsewhere.
    #[allow(dead_code)]
    None,
}

pub struct Reply {
    pub effects: Vec<Effect>,
    pub response: Response,
}

impl Reply {
    /// A result payload, serialized once right here.
    pub fn result<T: Serialize>(value: &T) -> Reply {
        let raw = serde_json::value::to_raw_value(value)
            .unwrap_or_else(|_| serde_json::value::to_raw_value(&()).expect("null serializes"));
        Reply {
            effects: Vec::new(),
            response: Response::Raw(raw),
        }
    }

    /// A pre-serialized result (the derived cache stores JSON text).
    pub fn raw(json: &str) -> Reply {
        match serde_json::value::RawValue::from_string(json.to_string()) {
            Ok(raw) => Reply {
                effects: Vec::new(),
                response: Response::Raw(raw),
            },
            Err(_) => Reply::null(),
        }
    }

    /// The ubiquitous `"result": null`.
    pub fn null() -> Reply {
        Reply::result(&serde_json::Value::Null)
    }

    #[allow(dead_code)] // cancellation / invalid-params commits
    pub fn error(code: i32, message: String) -> Reply {
        Reply {
            effects: Vec::new(),
            response: Response::Error { code, message },
        }
    }

    /// Prepend-free builder: effects transmit in `with` order.
    pub fn with(mut self, effect: Effect) -> Reply {
        self.effects.push(effect);
        self
    }
}

/// Ship a reply: effects in order, then the response for `id` (requests
/// only — notifications pass `None`).
pub fn transmit(
    link: &ClientLink,
    refresh: &tokio::sync::mpsc::UnboundedSender<String>,
    root: &str,
    id: Option<i64>,
    reply: Reply,
) {
    for effect in reply.effects {
        match effect {
            Effect::ShowMessage { level, text } => link.show_message(level, text),
            Effect::ShowDocument(params) => link.send_request("window/showDocument", params),
            Effect::ApplyEdit(params) => link.send_request("workspace/applyEdit", params),
            Effect::PublishDiagnostics { uri, items } => link.publish_diagnostics(uri, items),
            Effect::Refresh => {
                let _ = refresh.send(root.to_string());
            }
        }
    }
    if let Some(id) = id {
        match reply.response {
            Response::Raw(raw) => link.respond(id, &raw),
            Response::Error { code, message } => link.respond_error(id, code, message),
            Response::None => {}
        }
    }
}
