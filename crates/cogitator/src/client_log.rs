//! Editor-visible tracing (05 §6.3 addendum): a tracing Layer that
//! forwards events to the client as `window/logMessage` notifications,
//! so every editor's own log view (:LspLog, the eglot events buffer,
//! output panels) shows what the server is doing with zero setup.
//!
//! The layer installs with the subscriber BEFORE the stdout writer
//! channel exists, so its sink arrives later through a OnceLock; its
//! verbosity is one atomic (no EnvFilter — the per-layer-filter rule
//! of 05 §6.3 stays intact), hot-tuned by cogitator.yml
//! `log.client_level` and `$/setTrace`. Events targeting
//! `cogitator::wire` (the writer task's own per-frame trace) never
//! forward — that would be a feedback loop.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

use tokio::sync::mpsc::UnboundedSender;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

/// 0 = off, 1 = error, 2 = warn, 3 = info, 4 = debug. TRACE never
/// forwards, whatever the setting.
pub mod level {
    pub const OFF: u8 = 0;
    pub const ERROR: u8 = 1;
    pub const WARN: u8 = 2;
    pub const INFO: u8 = 3;
    pub const DEBUG: u8 = 4;
}

static SINK: OnceLock<UnboundedSender<Vec<u8>>> = OnceLock::new();
static LEVEL: AtomicU8 = AtomicU8::new(level::INFO);

/// Hand the layer the writer channel once the server built it.
pub fn set_sink(sender: UnboundedSender<Vec<u8>>) {
    let _ = SINK.set(sender);
}

pub fn set_level(new: u8) {
    LEVEL.store(new.min(level::DEBUG), Ordering::Relaxed);
}

pub fn current_level() -> u8 {
    LEVEL.load(Ordering::Relaxed)
}

/// The LSP `$/setTrace` / initialize `trace` values. "off" still
/// surfaces errors — the protocol tunes verbosity; only cogitator.yml
/// `log.client_level: off` blinds the stream entirely.
pub fn level_from_trace_value(value: &str) -> Option<u8> {
    Some(match value {
        "off" => level::ERROR,
        "messages" => level::INFO,
        "verbose" => level::DEBUG,
        _ => return None,
    })
}

/// cogitator.yml `log.client_level` / `$/setTrace` value names.
pub fn level_from_name(name: &str) -> Option<u8> {
    Some(match name {
        "off" => level::OFF,
        "error" => level::ERROR,
        "warn" => level::WARN,
        "info" => level::INFO,
        "debug" => level::DEBUG,
        _ => return None,
    })
}

pub struct ClientLogLayer;

impl<S: Subscriber> Layer<S> for ClientLogLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let Some(sink) = SINK.get() else { return };
        let metadata = event.metadata();
        if metadata.target() == "cogitator::wire" {
            return; // the writer task's own frame trace: never loop
        }
        let needed = match *metadata.level() {
            Level::ERROR => level::ERROR,
            Level::WARN => level::WARN,
            Level::INFO => level::INFO,
            Level::DEBUG => level::DEBUG,
            Level::TRACE => return,
        };
        if LEVEL.load(Ordering::Relaxed) < needed {
            return;
        }
        let mut text = MessageText::default();
        event.record(&mut text);
        let message_type = match needed {
            level::ERROR => cogitator_lsp::message_type::ERROR,
            level::WARN => cogitator_lsp::message_type::WARNING,
            level::INFO => cogitator_lsp::message_type::INFO,
            _ => cogitator_lsp::message_type::LOG,
        };
        let line = format!("{}: {}", metadata.target(), text.finish());
        if let Ok(frame) =
            cogitator_lsp::rpc::encode_frame_vec(&cogitator_lsp::log_message(message_type, line))
        {
            let _ = sink.send(frame);
        }
    }
}

/// "message field first, then k=v pairs" — compact one-line rendering.
#[derive(Default)]
struct MessageText {
    message: String,
    fields: String,
}

impl MessageText {
    fn finish(self) -> String {
        match (self.message.is_empty(), self.fields.is_empty()) {
            (false, false) => format!("{} ({})", self.message, self.fields.trim_end()),
            (false, true) => self.message,
            (true, _) => self.fields.trim_end().to_string(),
        }
    }
}

impl Visit for MessageText {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        use std::fmt::Write as _;
        if field.name() == "message" {
            let _ = write!(self.message, "{value:?}");
            // debug-formatting a &str message keeps its quotes; strip them
            if self.message.starts_with('"') && self.message.ends_with('"') {
                self.message = self.message[1..self.message.len() - 1].to_string();
            }
        } else {
            let _ = write!(self.fields, "{}={value:?} ", field.name());
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        use std::fmt::Write as _;
        if field.name() == "message" {
            self.message.push_str(value);
        } else {
            let _ = write!(self.fields, "{}={value} ", field.name());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::layer::SubscriberExt;

    fn drain(rx: &mut tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) -> Vec<serde_json::Value> {
        let mut out = Vec::new();
        while let Ok(frame) = rx.try_recv() {
            let text = String::from_utf8(frame).unwrap();
            let body = text.split("\r\n\r\n").nth(1).unwrap();
            out.push(serde_json::from_str(body).unwrap());
        }
        out
    }

    #[test]
    fn forwards_gated_events_as_log_messages() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        set_sink(tx);
        let subscriber = tracing_subscriber::registry().with(ClientLogLayer);

        tracing::subscriber::with_default(subscriber, || {
            set_level(level::INFO);
            tracing::info!(models = 9, "context refreshed");
            tracing::debug!("hidden at info");
            tracing::trace!(target: "cogitator::wire", "frame write");
            tracing::error!("boom");

            set_level(level::DEBUG);
            tracing::debug!(target: "cogitator::requests", method = "hover", "handled");

            set_level(level::OFF);
            tracing::error!("silenced");
        });

        let frames = drain(&mut rx);
        assert_eq!(frames.len(), 3, "{frames:?}");
        assert!(
            frames.iter().all(|f| f["method"] == "window/logMessage"),
            "{frames:?}"
        );
        assert!(
            frames[0]["params"]["message"]
                .as_str()
                .is_some_and(|m| m.contains("context refreshed") && m.contains("models=9")),
            "{frames:?}"
        );
        assert_eq!(frames[0]["params"]["type"], 3);
        assert_eq!(frames[1]["params"]["type"], 1, "error maps to 1");
        assert!(
            frames[2]["params"]["message"]
                .as_str()
                .is_some_and(|m| m.contains("cogitator::requests") && m.contains("handled")),
            "{frames:?}"
        );
        assert_eq!(frames[2]["params"]["type"], 4, "debug maps to Log");
        set_level(level::INFO); // leave the global tidy for other tests
    }
}
