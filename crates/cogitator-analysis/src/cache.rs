//! Project scan cache (specs/rust-port/05 §6.2 incremental-refresh fast
//! path): per-file artifacts keyed by (mtime, size), so a context refresh
//! re-reads and re-parses only the files that actually changed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::SystemTime;

type Stamp = (SystemTime, u64);

/// One artifact kind cached per source file. The parse closure stays pure
/// (text in, artifact out); this type owns the filesystem boundary.
#[derive(Debug)]
pub struct FileCache<T> {
    entries: HashMap<PathBuf, (Stamp, Arc<T>)>,
    /// Paths touched by the current build — used to evict deleted files.
    touched: Vec<PathBuf>,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl<T> Default for FileCache<T> {
    fn default() -> Self {
        FileCache {
            entries: HashMap::new(),
            touched: Vec::new(),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }
}

impl<T> FileCache<T> {
    /// Return the cached artifact for `path` when its stamp is unchanged;
    /// otherwise read + parse + store. `None` when the file is unreadable or
    /// the parse declines (per-file degradation, 05 §2).
    pub fn get_or_parse(
        &mut self,
        path: &Path,
        parse: impl FnOnce(&str, &Path) -> Option<T>,
    ) -> Option<Arc<T>> {
        let stamp = stamp(path)?;
        self.touched.push(path.to_path_buf());
        if let Some((cached_stamp, value)) = self.entries.get(path) {
            if *cached_stamp == stamp {
                self.hits.fetch_add(1, Ordering::Relaxed);
                return Some(value.clone());
            }
        }
        self.misses.fetch_add(1, Ordering::Relaxed);
        let text = std::fs::read_to_string(path).ok()?;
        let value = Arc::new(parse(&text, path)?);
        self.entries
            .insert(path.to_path_buf(), (stamp, value.clone()));
        Some(value)
    }

    /// Drop entries not touched since the last sweep (deleted/moved files).
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn sweep(&mut self) {
        let touched: std::collections::HashSet<_> = self.touched.drain(..).collect();
        self.entries.retain(|path, _| touched.contains(path));
    }

    pub fn stats(&self) -> (u64, u64) {
        (
            self.hits.load(Ordering::Relaxed),
            self.misses.load(Ordering::Relaxed),
        )
    }
}

fn stamp(path: &Path) -> Option<Stamp> {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

impl<T: serde::Serialize + serde::de::DeserializeOwned + Clone> FileCache<T> {
    /// Snapshot for on-disk persistence (03 §10): stamps + owned values.
    pub fn export(&self) -> Vec<(std::path::PathBuf, Stamp, T)> {
        self.entries
            .iter()
            .map(|(path, (stamp, value))| (path.clone(), *stamp, (**value).clone()))
            .collect()
    }

    /// Seed from a persisted snapshot. A seeded entry with a stale stamp
    /// self-heals through the ordinary (mtime, size) check on next use.
    pub fn seed(&mut self, items: Vec<(std::path::PathBuf, Stamp, T)>) {
        for (path, stamp, value) in items {
            self.entries
                .insert(path, (stamp, std::sync::Arc::new(value)));
        }
    }
}
