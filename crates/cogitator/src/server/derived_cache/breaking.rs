//! The breaking-change surface of the derived cache: the last report
//! per model uri, computed on open/save off the dispatcher task. The
//! push and pull diagnostics and the code lens read it; nothing here
//! computes. A per-uri ticket keeps an older job from overwriting a
//! newer one's answer.

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use cogitator_analysis::features::breaking::BreakingReport;

use super::DerivedCache;

/// uri → (latest ticket handed out, its report once finished).
type Reports = HashMap<String, (u64, Option<Arc<BreakingReport>>)>;

#[derive(Default)]
pub struct BreakingSlot {
    next: std::sync::atomic::AtomicU64,
    map: std::sync::Mutex<Reports>,
}

impl DerivedCache {
    /// A ticket for a new computation of `uri`; older tickets lose.
    pub fn breaking_begin(&self, uri: &str) -> u64 {
        let ticket = self.breaking.next.fetch_add(1, Ordering::Relaxed) + 1;
        let mut map = self.breaking.map.lock().unwrap_or_else(|e| e.into_inner());
        let entry = map.entry(uri.to_string()).or_insert((0, None));
        entry.0 = ticket;
        ticket
    }

    /// Store `report` when `ticket` is still the latest for `uri`; the
    /// report that was there before, or `None` when the ticket lost.
    pub fn breaking_finish(
        &self,
        uri: &str,
        ticket: u64,
        report: Option<Arc<BreakingReport>>,
    ) -> Option<Option<Arc<BreakingReport>>> {
        let mut map = self.breaking.map.lock().unwrap_or_else(|e| e.into_inner());
        let entry = map.get_mut(uri)?;
        if entry.0 != ticket {
            return None;
        }
        Some(std::mem::replace(&mut entry.1, report))
    }

    /// Which baseline the preview has for the project at `root`, from
    /// the LAST loaded artifact set (no fs: dbt.status is inline).
    pub fn breaking_baseline(&self, root: &std::path::Path) -> &'static str {
        let map = self.artifacts.lock().expect("artifacts cache lock");
        match map.get(root) {
            None => "notReadYet",
            Some((_, set)) if set.catalog.is_some() => "catalog",
            Some((_, set)) if set.manifest.is_some() => "manifest",
            Some(_) => "none",
        }
    }

    /// The last finished report for `uri`.
    pub fn breaking_report(&self, uri: &str) -> Option<Arc<BreakingReport>> {
        let map = self.breaking.map.lock().unwrap_or_else(|e| e.into_inner());
        map.get(uri).and_then(|(_, report)| report.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cogitator_analysis::features::breaking::Status;

    #[test]
    fn a_newer_ticket_wins() {
        let cache = DerivedCache::default();
        let report = |m: &str| {
            Some(Arc::new(BreakingReport::without(
                m,
                Status::NoBaseline,
                None,
            )))
        };
        let old = cache.breaking_begin("u");
        let new = cache.breaking_begin("u");
        assert!(cache.breaking_finish("u", old, report("old")).is_none());
        assert!(cache.breaking_finish("u", new, report("new")).is_some());
        assert_eq!(cache.breaking_report("u").unwrap().model, "new");
        assert!(cache.breaking_finish("other", 1, report("x")).is_none());
    }
}
