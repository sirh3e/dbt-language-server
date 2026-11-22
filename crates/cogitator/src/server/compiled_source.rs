//! Hover and definition on a compiled SQL file (`target/compiled/…`,
//! `target/run/…`): the line under the cursor mapped back to the Jinja
//! source it came from (`features::compiled_line_map`), read off the
//! dispatcher.

use std::path::{Path, PathBuf};

use cogitator_analysis::features::{self, SourceLines};
use cogitator_analysis::DbtContext;
use cogitator_lsp::{HoverResult, Location, Position};

use crate::server::reply::Reply;
use crate::server::snapshot::Snapshot;

/// The source file of a compiled document, when `uri` is one.
pub fn source_of(ctx: &DbtContext, uri: &str) -> Option<PathBuf> {
    let path = cogitator_lsp::uri_to_fs_path(uri)?;
    let root = ctx.project_root.as_deref()?;
    features::compiled_source_path(root, &ctx.project.name, Path::new(&path))
}

/// The source lines of the compiled line at `pos`, the source file and
/// its text.
fn mapped(snap: &Snapshot, uri: &str, pos: Position) -> Option<(SourceLines, PathBuf, String)> {
    let source = source_of(&snap.ctx, uri)?;
    let source_text = std::fs::read_to_string(&source).ok()?;
    let compiled = match snap.doc.as_deref() {
        Some(doc) => doc.text.to_string(),
        None => std::fs::read_to_string(cogitator_lsp::uri_to_fs_path(uri)?).ok()?,
    };
    let map = features::compiled_line_map(&source_text, &compiled);
    let lines = *map.get(pos.line as usize)?;
    Some((lines, source, source_text))
}

fn shown(ctx: &DbtContext, path: &Path) -> String {
    ctx.project_root
        .as_deref()
        .and_then(|root| path.strip_prefix(root).ok())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// textDocument/hover on a compiled file: where the line came from, then
/// whatever the SQL hover says.
pub fn hover(snap: &Snapshot, uri: &str, pos: Position) -> Reply {
    let mut parts = Vec::new();
    if let Some((lines, source, text)) = mapped(snap, uri, pos) {
        let source_uri = cogitator_lsp::fs_path_to_uri(&source);
        parts.push(features::compiled_hover(
            lines,
            &text,
            &source_uri,
            &shown(&snap.ctx, &source),
        ));
    }
    if let Some(doc) = snap.doc.as_deref() {
        let sql = features::hover_with_lineage(&snap.ctx, doc, pos, None);
        if !sql.is_empty() {
            parts.push(sql);
        }
    }
    Reply::result(&HoverResult {
        contents: parts.join("\n\n---\n\n"),
    })
}

/// textDocument/definition on a compiled file: the source line.
pub fn definition(snap: &Snapshot, uri: &str, pos: Position) -> Option<Location> {
    let (lines, source, _) = mapped(snap, uri, pos)?;
    Some(features::compiled_definition(
        lines,
        &cogitator_lsp::fs_path_to_uri(&source),
    ))
}
