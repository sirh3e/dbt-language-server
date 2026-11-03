//! The stamped target/ artifact surface of the derived cache (03
//! §10): run_results/manifest/sources re-read only when their
//! (mtime, size) changed, with a fs-free accessor for the inline
//! push path.

use std::path::Path;
use std::sync::Arc;

use cogitator_analysis::artifacts::{ArtifactPaths, ArtifactSet};

use super::{stamp, DerivedCache, FileStamp};

impl DerivedCache {
    /// The artifact set for `root`, re-reading only files whose stamp
    /// changed; files over `cap_bytes` or unparseable load as None (a
    /// trace, never a user-facing failure — 03 §10).
    pub fn artifacts(&self, root: &Path, cap_bytes: u64) -> ArtifactSet {
        let paths = ArtifactPaths::under(root);
        let files = [
            &paths.run_results,
            &paths.manifest,
            &paths.sources,
            &paths.catalog,
        ];
        let current = files.map(|p| stamp(p));
        {
            let map = self.artifacts.lock().expect("artifacts cache lock");
            if let Some((stamps, set)) = map.get(root) {
                if *stamps == current {
                    self.artifacts_stats.hit("artifacts");
                    return set.clone();
                }
            }
        }
        self.artifacts_stats.miss("artifacts");
        let read = |path: &Path, stamp: &Option<FileStamp>| -> Option<String> {
            let (_, _, size) = (*stamp)?;
            if size > cap_bytes {
                tracing::debug!(
                    path = %path.display(),
                    size,
                    "artifact over limits.artifact_mib; skipped"
                );
                return None;
            }
            std::fs::read_to_string(path).ok()
        };
        let trace_err = |e: cogitator_error::guidance::Guidance| {
            tracing::debug!(error = %e.to_message(), "artifact unreadable; feature off");
        };
        let set = ArtifactSet {
            run_results: read(&paths.run_results, &current[0]).and_then(|t| {
                cogitator_analysis::artifacts::run_results::parse(&t, &paths.run_results)
                    .map_err(trace_err)
                    .ok()
                    .map(Arc::new)
            }),
            manifest: read(&paths.manifest, &current[1]).and_then(|t| {
                cogitator_analysis::artifacts::manifest::parse(&t, &paths.manifest)
                    .map_err(trace_err)
                    .ok()
                    .map(Arc::new)
            }),
            freshness: read(&paths.sources, &current[2]).and_then(|t| {
                cogitator_analysis::artifacts::freshness::parse(&t, &paths.sources)
                    .map_err(trace_err)
                    .ok()
                    .map(Arc::new)
            }),
            catalog: read(&paths.catalog, &current[3]).and_then(|t| {
                cogitator_analysis::artifacts::catalog::parse(&t, &paths.catalog)
                    .map_err(trace_err)
                    .ok()
                    .map(Arc::new)
            }),
        };
        self.artifacts
            .lock()
            .expect("artifacts cache lock")
            .insert(root.to_path_buf(), (current, set.clone()));
        set
    }

    /// The last loaded set WITHOUT touching the filesystem — for the
    /// inline push path (05 §6.1: the dispatcher task never does fs).
    pub fn artifacts_cached(&self, root: &Path) -> ArtifactSet {
        self.artifacts
            .lock()
            .expect("artifacts cache lock")
            .get(root)
            .map(|(_, set)| set.clone())
            .unwrap_or_default()
    }
}
