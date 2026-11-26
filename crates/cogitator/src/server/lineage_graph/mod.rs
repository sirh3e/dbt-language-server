//! The project-wide column-lineage cache behind `dbt.columnLineageGraph`:
//! every model's typed lineage, kept per project root and keyed by what it
//! was computed over (`stamps`), so an edit recomputes the edited model
//! only. Filled by worker threads off the request path (`store`) and kept
//! on disk beside the scan cache (`persist`), so a restart computes
//! nothing that did not change. Downstream edges are not stored: the walk
//! derives them from the children's lineage.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use serde::Serialize;

mod persist;
pub mod stamps;
mod store;
#[cfg(test)]
mod tests;

pub use store::{Compute, Store};

/// Every project root's store.
#[derive(Default)]
pub struct Stores {
    roots: Mutex<HashMap<PathBuf, Arc<Store>>>,
}

/// The lineage graph's line in dbt.cache / dbt.status.
#[derive(Debug, Default, Serialize)]
pub struct GraphCacheReport {
    /// Models with a cached lineage.
    pub entries: usize,
    /// Lineages answered from the cache.
    pub hits: u64,
    /// Lineages computed.
    pub misses: u64,
    /// Entries read from disk when a store opened.
    pub loaded: u64,
    /// Models queued or being computed now.
    pub pending: usize,
    /// Times the cache was written to disk.
    pub saves: u64,
}

/// How many threads fill one store.
fn workers() -> usize {
    std::thread::available_parallelism()
        .map_or(2, |n| n.get())
        .clamp(1, 8)
}

impl Stores {
    /// The store of `root`, opened (and read from disk) on first use.
    /// Reads a file: offloaded callers only.
    pub fn store(&self, root: &Path) -> Arc<Store> {
        let mut roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
        roots
            .entry(root.to_path_buf())
            .or_insert_with(|| Arc::new(Store::open(root.to_path_buf(), workers())))
            .clone()
    }

    pub fn report(&self) -> GraphCacheReport {
        let roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
        roots
            .values()
            .fold(GraphCacheReport::default(), |mut r, store| {
                r.entries += store.entries();
                r.pending += store.pending();
                r.hits += store.stats.hits.load(Ordering::Relaxed);
                r.misses += store.stats.computed.load(Ordering::Relaxed);
                r.loaded += store.stats.loaded.load(Ordering::Relaxed);
                r.saves += store.stats.saves.load(Ordering::Relaxed);
                r
            })
    }

    /// Forgets every root's entries, on disk too.
    pub fn clear(&self) {
        let roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
        for store in roots.values() {
            store.clear();
        }
    }
}
