//! `{% docs %}` block details (03 §6.4): the docs blocks kept ON the
//! context, so `doc('…')` gets hover, definition, completion and a
//! dangling diagnostic — not only the build-time description
//! expansion that used to throw the map away.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use cogitator_lsp::{Position, Range};
use cogitator_parser::jinja_scan;
use cogitator_schemas::project::ProjectFile;

use super::models::DocsCache;
use crate::util;

/// One `{% docs name %}` block: where it lives and what it says.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocDetails {
    pub uri: PathBuf,
    /// Zero-width range at the block's name identifier.
    pub range: Range,
    pub content: String,
    pub project_name: String,
}

/// Every docs block under one project's docs paths, cached per file.
/// Names are project-flat, as `doc('name')` references them; the
/// caller keeps the FIRST definition on collision, so the own project
/// (scanned first) wins over installed packages.
pub(super) fn build_doc_details(
    root: &Path,
    project: &ProjectFile,
    cache: &mut DocsCache,
) -> HashMap<String, DocDetails> {
    let mut map = HashMap::new();
    for rel in &project.docs_paths {
        let dir = root.join(rel);
        if std::fs::read_dir(&dir).is_err() {
            continue;
        }
        for file in util::walk_filepath(&dir, ".md") {
            let Some(blocks) =
                cache.get_or_parse(&file, |text, _| Some(jinja_scan::extract_docs_blocks(text)))
            else {
                continue;
            };
            for b in blocks.iter() {
                map.entry(b.name.clone()).or_insert_with(|| DocDetails {
                    uri: PathBuf::from(&file),
                    range: Range::zero_width(Position {
                        line: b.start.0,
                        character: b.start.1,
                    }),
                    content: b.content.clone(),
                    project_name: project.name.clone(),
                });
            }
        }
    }
    map
}
