//! The shell of inline column lineage (`inlay.lineage`): the dispatcher
//! only probes the lineage cache; on a miss one blocking task per uri
//! computes it (`typed_lineage_for`, the cache everyone shares) and asks
//! the client to re-request its inlay hints once it lands, so typing
//! never waits for lineage.

use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, OnceLock};

use cogitator_analysis::features::lineage_inlay::{
    lineage_inlay_hints, LineageInlayOptions, ProjectLocator, DEFAULT_MAX_LEN,
};
use cogitator_analysis::lineage::ModelLineage;
use cogitator_lsp::{AnyInlayHint, Range};

use super::super::client_link::ClientLink;
use super::super::derived_cache::DerivedCache;
use super::super::snapshot::Snapshot;
use super::super::SharedToggles;

/// What cogitator.yml says now.
pub fn options(toggles: &SharedToggles) -> LineageInlayOptions {
    let max_len = toggles.inlay_lineage_max_len.load(Ordering::Relaxed);
    LineageInlayOptions {
        enabled: !toggles.inlay_lineage_off.load(Ordering::Relaxed),
        max_len: match usize::try_from(max_len) {
            Ok(0) | Err(_) => DEFAULT_MAX_LEN,
            Ok(len) => len,
        },
    }
}

/// Whether `uri` is a document the hints are for: a model's SQL.
pub fn wanted(uri: &str, options: LineageInlayOptions) -> bool {
    options.enabled && uri.ends_with(".sql")
}

/// The hints from lineage the dispatcher found cached. Offloaded: the
/// parts' locations read the upstream files.
pub fn hints(
    snap: &Snapshot,
    lineage: Option<&ModelLineage>,
    range: Range,
    options: LineageInlayOptions,
) -> Vec<AnyInlayHint> {
    let (Some(doc), Some(lineage)) = (snap.doc.as_deref(), lineage) else {
        return Vec::new();
    };
    let read = |path: &std::path::Path| std::fs::read_to_string(path).ok();
    let mut locator = ProjectLocator::new(&snap.ctx, &read);
    lineage_inlay_hints(&doc.text, lineage, range, options, &mut locator)
        .into_iter()
        .map(AnyInlayHint::Parted)
        .collect()
}

/// Uris with a lineage computation in flight: one at a time per uri, so
/// a burst of edits queues one, not one per keystroke. The refresh it
/// ends with brings a request for the newest text, which starts the next.
fn warming() -> &'static Mutex<HashSet<String>> {
    static WARMING: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    WARMING.get_or_init(Default::default)
}

/// Computes the lineage of `uri` off the dispatcher and, when it gives
/// anything to place, sends `workspace/inlayHint/refresh` (if the client
/// takes it; otherwise its next request finds the cache warm).
pub fn warm(
    snap: Snapshot,
    derived: Arc<DerivedCache>,
    uri: String,
    artifact_cap: u64,
    link: ClientLink,
    refresh: bool,
) {
    if !warming()
        .lock()
        .expect("lineage warming lock")
        .insert(uri.clone())
    {
        return;
    }
    tokio::task::spawn_blocking(move || {
        let _span = tracing::debug_span!("lineage_hints.warm", uri = %uri).entered();
        let _done = Warming(&uri); // released even if the computation panics
        let lineage = super::lineage::typed_lineage_for(&snap, &derived, &uri, artifact_cap);
        let placed = lineage
            .as_ref()
            .as_ref()
            .is_some_and(|l| l.columns.iter().any(|c| c.end.is_some()));
        if placed && refresh {
            link.send_bare_request("workspace/inlayHint/refresh");
        }
    });
}

/// Takes the uri out of [`warming`] when the computation ends.
struct Warming<'a>(&'a str);

impl Drop for Warming<'_> {
    fn drop(&mut self) {
        if let Ok(mut set) = warming().lock() {
            set.remove(self.0);
        }
    }
}
