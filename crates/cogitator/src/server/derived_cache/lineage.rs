//! The typed-lineage surface of the derived cache: one entry per uri,
//! keyed by (document generation, plan key). Offloaded requests fill it;
//! the hover path only probes it.

use std::sync::Arc;

use cogitator_analysis::lineage::ModelLineage;

use super::DerivedCache;

impl DerivedCache {
    /// The cached typed lineage for `uri`, computed on a miss.
    pub fn typed_lineage(
        &self,
        uri: &str,
        generation: u64,
        key: u64,
        revision: u64,
        compute: impl FnOnce() -> Option<ModelLineage>,
    ) -> Arc<Option<ModelLineage>> {
        {
            let map = self.typed_lineage.lock().expect("typed lineage lock");
            if let Some((cached, lineage)) = map.get(uri) {
                if *cached == (generation, key, revision) {
                    self.lineage_stats.hit("lineage");
                    return lineage.clone();
                }
            }
        }
        self.lineage_stats.miss("lineage");
        let lineage = Arc::new(compute());
        self.typed_lineage
            .lock()
            .expect("typed lineage lock")
            .insert(
                uri.to_string(),
                ((generation, key, revision), lineage.clone()),
            );
        lineage
    }

    /// The lineage of `uri` at this document generation if one is cached
    /// (typed first), never computing: the dispatcher path's only access.
    /// A generation match means the text it was computed over is unchanged.
    /// The schema `revision` must match too: lineage computed before the
    /// project's schemas were published (a request right after startup)
    /// has no upstream columns, and is computed again once they are.
    pub fn lineage_cached(
        &self,
        uri: &str,
        generation: u64,
        revision: u64,
    ) -> Option<Arc<Option<ModelLineage>>> {
        let typed = self.typed_lineage.lock().expect("typed lineage lock");
        if let Some(((cached, _, at), lineage)) = typed.get(uri) {
            if *cached == generation && *at == revision {
                return Some(lineage.clone());
            }
        }
        drop(typed);
        let plain = self.lineage.lock().expect("lineage cache lock");
        let (cached, lineage) = plain.get(uri)?;
        (*cached == generation).then(|| lineage.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cogitator_analysis::lineage::ModelLineage;

    fn empty() -> Option<ModelLineage> {
        Some(ModelLineage {
            columns: Vec::new(),
            origin: None,
        })
    }

    #[test]
    fn typed_entries_key_on_generation_and_plan_key() {
        let cache = DerivedCache::default();
        let first = cache.typed_lineage("u", 1, 7, 1, empty);
        let hit = cache.typed_lineage("u", 1, 7, 1, || panic!("must not compute"));
        assert!(Arc::ptr_eq(&first, &hit));
        let moved = cache.typed_lineage("u", 1, 8, 1, empty);
        assert!(!Arc::ptr_eq(&first, &moved), "a new plan key recomputes");
    }

    #[test]
    fn the_probe_misses_lineage_of_an_older_schema_revision() {
        let cache = DerivedCache::default();
        // computed before the schemas were published (revision 0)
        cache.typed_lineage("u", 3, 7, 0, empty);
        assert!(cache.lineage_cached("u", 3, 0).is_some());
        assert!(cache.lineage_cached("u", 3, 1).is_none(), "published since");
    }

    #[test]
    fn the_probe_never_computes_and_matches_the_generation() {
        let cache = DerivedCache::default();
        assert!(cache.lineage_cached("u", 1, 1).is_none());
        cache.typed_lineage("u", 3, 7, 1, empty);
        assert!(cache.lineage_cached("u", 3, 1).is_some());
        assert!(cache.lineage_cached("u", 4, 1).is_none());
        cache.lineage("v", 2, empty);
        assert!(
            cache.lineage_cached("v", 2, 1).is_some(),
            "plain entries too"
        );
    }
}
