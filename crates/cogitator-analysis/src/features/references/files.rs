//! The file side of project-wide navigation: which files a search reads,
//! which project a file belongs to, and how a bare macro name resolves
//! from it. Reading goes through a `read` closure so the algorithms stay
//! testable; the public entry points pass [`Reader::disk`].

use std::path::{Path, PathBuf};

use crate::context::MacroDetails;

use super::super::DbtContext;

/// File text by path: the open editor buffer for its own file (so an
/// unsaved edit is what gets searched), disk for everything else.
pub(crate) struct Reader<'a> {
    open: Option<(PathBuf, &'a str)>,
    read: &'a dyn Fn(&Path) -> Option<String>,
}

fn read_disk(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

impl<'a> Reader<'a> {
    pub(crate) fn disk(open: Option<(&str, &'a str)>) -> Self {
        Reader::with(open, &read_disk)
    }

    pub(crate) fn with(
        open: Option<(&str, &'a str)>,
        read: &'a dyn Fn(&Path) -> Option<String>,
    ) -> Self {
        Reader {
            open: open.and_then(|(uri, text)| Some((path_of(uri)?, text))),
            read,
        }
    }

    pub(crate) fn read(&self, path: &Path) -> Option<String> {
        match &self.open {
            Some((open, text)) if open == path => Some(text.to_string()),
            _ => (self.read)(path),
        }
    }
}

/// A `file://` uri (or a bare path) as a filesystem path.
pub(crate) fn path_of(uri: &str) -> Option<PathBuf> {
    if uri.is_empty() {
        return None;
    }
    Some(PathBuf::from(
        cogitator_lsp::uri_to_fs_path(uri).unwrap_or_else(|| uri.to_string()),
    ))
}

pub(crate) fn is_yaml(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "yml" || e == "yaml")
}

/// The project a file belongs to: a package when one of its macro files
/// is this file, else the root project.
pub(crate) fn project_of_path(ctx: &DbtContext, path: &Path) -> String {
    ctx.macro_detail_map
        .iter()
        .find(|(_, macros)| macros.values().any(|m| m.uri == path))
        .map(|(project, _)| project.clone())
        .unwrap_or_else(|| ctx.project.name.clone())
}

/// A macro call as dbt resolves it (03 §5): a qualified `pkg.name` in
/// that package; a bare name in the calling file's own project first,
/// then the root project, then any installed package.
pub(crate) fn resolve_macro<'a>(
    ctx: &'a DbtContext,
    from_project: &str,
    qualifier: Option<&str>,
    name: &str,
) -> Option<&'a MacroDetails> {
    let in_project = |p: &str| ctx.macro_detail_map.get(p).and_then(|m| m.get(name));
    match qualifier {
        Some(q) => in_project(q),
        None => in_project(from_project)
            .or_else(|| in_project(&ctx.project.name))
            .or_else(|| {
                let mut projects: Vec<&String> = ctx.macro_detail_map.keys().collect();
                projects.sort();
                projects.into_iter().find_map(|p| in_project(p))
            }),
    }
}

/// Every distinct macro file of every scanned project, with its project.
pub(crate) fn macro_files(ctx: &DbtContext) -> Vec<(PathBuf, String)> {
    let mut out: Vec<(PathBuf, String)> = ctx
        .macro_detail_map
        .iter()
        .flat_map(|(project, macros)| macros.values().map(|m| (m.uri.clone(), project.clone())))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// The root project's properties YAML files: every `.yml`/`.yaml` under
/// its model, macro, seed and snapshot paths.
pub(crate) fn yaml_files(ctx: &DbtContext) -> Vec<PathBuf> {
    let Some(root) = ctx.project_root.as_deref() else {
        return Vec::new();
    };
    let p = &ctx.project;
    let mut out: Vec<PathBuf> = p
        .model_paths
        .iter()
        .chain(&p.macro_paths)
        .chain(&p.seed_paths)
        .chain(&p.snapshot_paths)
        .flat_map(|rel| {
            let dir = root.join(rel);
            let mut files = crate::util::walk_filepath(&dir, ".yml");
            files.extend(crate::util::walk_filepath(&dir, ".yaml"));
            files
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

/// The files of graph nodes, empty placeholders left out, sorted.
pub(crate) fn node_files<'a>(
    nodes: impl Iterator<Item = &'a crate::graph::NodeData>,
) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = nodes
        .map(|n| n.uri.clone())
        .filter(|p| !p.as_os_str().is_empty())
        .collect();
    out.sort();
    out.dedup();
    out
}
