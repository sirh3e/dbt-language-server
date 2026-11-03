//! dbt artifact readers (03 §10): target/run_results.json, a minimal
//! manifest.json index and sources.json, parsed LENIENTLY — every
//! field optional, unknown fields ignored, no schema-version gating.
//! dbt reshapes these files across versions; artifacts must degrade
//! feature-off, never break core diagnostics.

pub mod freshness;
pub mod manifest;
pub mod run_results;
pub mod status;

use std::path::{Path, PathBuf};

/// Where the artifacts live under a project root. `target-path`
/// overrides are not read today (the scan does not keep them); the
/// dbt default is what the rest of the server assumes too.
#[derive(Debug, Clone)]
pub struct ArtifactPaths {
    pub run_results: PathBuf,
    pub manifest: PathBuf,
    pub sources: PathBuf,
}

impl ArtifactPaths {
    pub fn under(root: &Path) -> Self {
        let target = root.join("target");
        ArtifactPaths {
            run_results: target.join("run_results.json"),
            manifest: target.join("manifest.json"),
            sources: target.join("sources.json"),
        }
    }
}

/// Everything the shell managed to read this round; any member may be
/// absent (missing file, over the size cap, or unparseable).
#[derive(Debug, Default, Clone)]
pub struct ArtifactSet {
    pub run_results: Option<std::sync::Arc<run_results::RunResults>>,
    pub manifest: Option<std::sync::Arc<manifest::ManifestIndex>>,
    pub freshness: Option<std::sync::Arc<freshness::SourcesFreshness>>,
}
