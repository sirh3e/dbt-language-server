//! The column-reach surface of the derived cache: every output column's
//! reach per model, computed on open/save off the dispatcher task; the
//! hover reads it and never computes. A per-model ticket keeps an older
//! job from overwriting a newer one's answer.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use cogitator_analysis::features::column_reach::ModelReach;

use super::DerivedCache;

/// model → (latest ticket handed out, its reach once finished).
type Reaches = HashMap<String, (u64, Option<Arc<ModelReach>>)>;

#[derive(Default)]
pub struct ReachSlot {
    next: AtomicU64,
    map: Mutex<Reaches>,
}

impl DerivedCache {
    /// A ticket for a new computation of `model`; older tickets lose.
    pub fn reach_begin(&self, model: &str) -> u64 {
        let ticket = self.reach.next.fetch_add(1, Ordering::Relaxed) + 1;
        let mut map = self.reach.map.lock().unwrap_or_else(|e| e.into_inner());
        map.entry(model.to_string()).or_insert((0, None)).0 = ticket;
        ticket
    }

    /// Store `reach` when `ticket` is still the latest for `model`; a
    /// failed computation keeps the previous answer.
    pub fn reach_finish(&self, model: &str, ticket: u64, reach: Option<Arc<ModelReach>>) {
        let mut map = self.reach.map.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = map.get_mut(model).filter(|e| e.0 == ticket) {
            if reach.is_some() {
                entry.1 = reach;
            }
        }
    }

    /// The last finished reach of `model`.
    pub fn reach_cached(&self, model: &str) -> Option<Arc<ModelReach>> {
        let map = self.reach.map.lock().unwrap_or_else(|e| e.into_inner());
        map.get(model).and_then(|(_, r)| r.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_tickets_win() {
        let cache = DerivedCache::default();
        let old = cache.reach_begin("m");
        let new = cache.reach_begin("m");
        let reach = |name: &str| {
            Some(Arc::new(ModelReach {
                model: name.into(),
                ..ModelReach::default()
            }))
        };
        cache.reach_finish("m", new, reach("new"));
        cache.reach_finish("m", old, reach("old"));
        assert_eq!(cache.reach_cached("m").unwrap().model, "new");
        cache.reach_finish("m", cache.reach_begin("m"), None);
        assert_eq!(cache.reach_cached("m").unwrap().model, "new");
    }
}
