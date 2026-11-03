//! The in-flight table behind concurrent request serving (05 §6.2):
//! slow/fs handlers decode + snapshot INLINE on the dispatcher task,
//! then run here — tokio::spawn for async work, spawn_blocking for
//! CPU/fs jobs — transmitting their own Reply when done. Every entry
//! carries an AbortHandle plus a `responded` guard, so `$/cancelRequest`
//! and completion race to exactly ONE frame per request id. Honest
//! caveat (also in the spec): aborting a blocking job only suppresses
//! its reply — the closure itself runs to completion off-thread.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use cogitator_lsp::error_codes;
use tokio::sync::mpsc;
use tracing::Instrument;

use super::client_link::ClientLink;
use super::reply::{transmit, Reply};
use super::test_hooks;

struct Entry {
    abort: tokio::task::AbortHandle,
    responded: Arc<AtomicBool>,
}

/// Dispatcher-owned (no lock): ids map to their in-flight work.
#[derive(Default)]
pub struct InFlight {
    map: HashMap<i64, Entry>,
}

/// Everything an offloaded task needs to ship its reply.
#[derive(Clone)]
pub struct OffloadLink {
    pub link: ClientLink,
    pub refresh_tx: mpsc::UnboundedSender<String>,
    pub root_path: String,
}

impl InFlight {
    /// Run a blocking (CPU/fs) job off the dispatcher task.
    pub fn spawn_blocking(
        &mut self,
        id: i64,
        name: &'static str,
        out: OffloadLink,
        job: impl FnOnce() -> Reply + Send + 'static,
    ) {
        self.retain_live();
        let responded = Arc::new(AtomicBool::new(false));
        let guard = responded.clone();
        let span = tracing::info_span!("offload", handler = name, id);
        let handle = tokio::spawn(
            async move {
                if let Some(delay) = test_hooks::delay_for(name) {
                    tokio::time::sleep(delay).await;
                }
                let reply = tokio::task::spawn_blocking(job).await;
                if let Ok(reply) = reply {
                    if !guard.swap(true, Ordering::SeqCst) {
                        transmit(&out.link, &out.refresh_tx, &out.root_path, Some(id), reply);
                    }
                }
            }
            .instrument(span),
        );
        self.map.insert(
            id,
            Entry {
                abort: handle.abort_handle(),
                responded,
            },
        );
    }

    /// `$/cancelRequest`: abort and answer RequestCancelled — unless the
    /// task already responded (the guard makes the race single-shot).
    pub fn cancel(&mut self, id: i64, link: &ClientLink) {
        if let Some(entry) = self.map.remove(&id) {
            entry.abort.abort();
            if !entry.responded.swap(true, Ordering::SeqCst) {
                link.respond_error(
                    id,
                    error_codes::REQUEST_CANCELLED,
                    "request cancelled by the client".to_string(),
                );
            }
        }
    }

    /// shutdown: nothing in flight may answer after the lifecycle ends.
    pub fn abort_all(&mut self) {
        for (_, entry) in self.map.drain() {
            entry.abort.abort();
            entry.responded.store(true, Ordering::SeqCst);
        }
    }

    /// Completed entries never remove themselves; keep the map bounded.
    fn retain_live(&mut self) {
        self.map.retain(|_, entry| !entry.abort.is_finished());
    }
}
