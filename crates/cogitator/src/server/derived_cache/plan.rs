//! The `dbt.queryPlan` surface of the derived cache: serialized plans
//! keyed by (uri, document generation, schema revision).

use std::sync::Arc;

use super::DerivedCache;

impl DerivedCache {
    /// The cached plan JSON for `uri`, computed on a miss. One entry per
    /// uri: an edit or a new schema revision replaces it in place.
    pub fn query_plan(
        &self,
        uri: &str,
        generation: u64,
        revision: u64,
        compute: impl FnOnce() -> Option<String>,
    ) -> Option<Arc<str>> {
        let key = (generation, revision);
        {
            let map = self.plans.lock().expect("plans cache lock");
            if let Some((cached, json)) = map.get(uri) {
                if *cached == key {
                    self.plans_stats.hit("plans");
                    return Some(json.clone());
                }
            }
        }
        self.plans_stats.miss("plans");
        let json: Arc<str> = Arc::from(compute()?.as_str());
        self.plans
            .lock()
            .expect("plans cache lock")
            .insert(uri.to_string(), (key, json.clone()));
        Some(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(cache: &DerivedCache, generation: u64, revision: u64, body: &str) -> Arc<str> {
        cache
            .query_plan("file:///m.sql", generation, revision, || {
                Some(body.to_string())
            })
            .expect("plan")
    }

    #[test]
    fn hits_until_the_generation_or_revision_moves() {
        let cache = DerivedCache::default();
        let first = plan(&cache, 1, 9, "{\"a\":1}");
        assert!(Arc::ptr_eq(&first, &plan(&cache, 1, 9, "{\"a\":2}")));
        assert_eq!(&*plan(&cache, 2, 9, "{\"a\":2}"), "{\"a\":2}");
        assert_eq!(&*plan(&cache, 2, 10, "{\"a\":3}"), "{\"a\":3}");
        let report = cache.report();
        assert_eq!(report.plans.entries, 1, "one entry per uri");
        assert_eq!((report.plans.hits, report.plans.misses), (1, 3));
    }

    #[test]
    fn a_failed_compute_is_not_cached() {
        let cache = DerivedCache::default();
        assert!(cache.query_plan("u", 1, 1, || None).is_none());
        assert_eq!(cache.report().plans.entries, 0);
    }
}
