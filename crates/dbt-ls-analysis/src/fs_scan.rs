//! The filesystem boundary of the scan side (05 §3: side-effect boundaries —
//! this module reads, everything downstream is pure over the results).

use std::path::{Path, PathBuf};

use crate::util;

/// One file read from disk: the only owner of scan-side text (08 §1.3 — the
/// extractors borrow from it and own only what enters the snapshot).
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: PathBuf,
    pub text: String,
}

/// Read every `ext` file under `root/<p>` for each relative path, in
/// deterministic walk order. Unreadable directories and files are skipped
/// (03 §5–§8 degradation).
pub fn read_sources(root: &Path, rel_paths: &[String], ext: &str) -> Vec<SourceFile> {
    rel_paths
        .iter()
        .map(|p| root.join(p))
        .filter(|dir| std::fs::read_dir(dir).is_ok())
        .flat_map(|dir| util::walk_filepath(&dir, ext))
        .filter_map(|path| {
            let text = std::fs::read_to_string(&path).ok()?;
            Some(SourceFile { path, text })
        })
        .collect()
}
