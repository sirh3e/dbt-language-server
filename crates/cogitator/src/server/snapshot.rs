//! The per-message snapshot handed to handlers (and later to offloaded
//! tasks): cheap Arc clones taken ON the dispatcher task — a watch
//! borrow guard never crosses an await, and receivers never leave the
//! dispatcher (the 05 §6.2 concurrency rule).

use std::sync::Arc;

use cogitator_analysis::{DbtContext, Document};

use super::ProfilesConfig;

pub struct Snapshot {
    pub ctx: Arc<DbtContext>,
    /// The request's document, resolved inline before any offload.
    pub doc: Option<Arc<Document>>,
    pub root_path: String,
    /// Cloned under a tight lock — handlers never touch the mutex.
    pub profiles: ProfilesConfig,
    #[allow(dead_code)] // status/fusion handlers migrate onto these
    pub fusion_path: String,
    #[allow(dead_code)]
    pub fusion_enabled: bool,
}
