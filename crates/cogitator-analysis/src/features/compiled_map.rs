//! Hover and definition on compiled SQL (FEATURE_REQUESTS, editor
//! experience): a line of `target/compiled/<project>/…` (or `target/run`)
//! mapped back to the Jinja source it came from.
//!
//! The map is a deterministic line alignment of the source and the
//! compiled text: both are normalized (trimmed, whitespace collapsed) and
//! diffed with the patience algorithm. Lines dbt copied through are
//! anchors and map exactly; a stretch of compiled lines between anchors
//! comes from the source lines between the same anchors — line for line
//! when both stretches are as long, else the whole source stretch (a
//! `{% for %}` that unrolled, a macro that expanded). Lines only the
//! compiled text has map to the source line before them.

use std::path::{Path, PathBuf};

use cogitator_lsp::{Location, Position, Range};
use similar::{capture_diff_slices, Algorithm, DiffOp};

/// The source lines (0-based, inclusive) one compiled line came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceLines {
    pub start: u32,
    pub end: u32,
    /// The line was copied through unchanged (modulo whitespace).
    pub exact: bool,
}

fn normalized(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect()
}

/// For every compiled line, the source lines it came from.
pub fn compiled_line_map(source: &str, compiled: &str) -> Vec<SourceLines> {
    let (src, out) = (normalized(source), normalized(compiled));
    let last = src.len().saturating_sub(1) as u32;
    let mut map = vec![
        SourceLines {
            start: 0,
            end: 0,
            exact: false
        };
        out.len()
    ];
    let around = |old_index: usize| {
        let line = (old_index.saturating_sub(1) as u32).min(last);
        SourceLines {
            start: line,
            end: line,
            exact: false,
        }
    };
    for op in capture_diff_slices(Algorithm::Patience, &src, &out) {
        match op {
            DiffOp::Equal {
                old_index,
                new_index,
                len,
            } => {
                for i in 0..len {
                    let line = (old_index + i) as u32;
                    map[new_index + i] = SourceLines {
                        start: line,
                        end: line,
                        exact: true,
                    };
                }
            }
            DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => {
                for i in 0..new_len {
                    map[new_index + i] = if old_len == new_len {
                        let line = (old_index + i) as u32;
                        SourceLines {
                            start: line,
                            end: line,
                            exact: false,
                        }
                    } else {
                        SourceLines {
                            start: old_index as u32,
                            end: (old_index + old_len - 1) as u32,
                            exact: false,
                        }
                    };
                }
            }
            DiffOp::Insert {
                old_index,
                new_index,
                new_len,
            } => {
                for i in 0..new_len {
                    map[new_index + i] = around(old_index);
                }
            }
            DiffOp::Delete { .. } => {}
        }
    }
    map
}

/// The source file of a compiled one: `<root>/target/{compiled,run}/
/// <package>/<path>` is `<root>/<path>` for the project itself and
/// `<root>/dbt_packages/<package>/<path>` for a package. `None` for a
/// file outside those directories.
pub fn compiled_source_path(root: &Path, project: &str, compiled: &Path) -> Option<PathBuf> {
    let target = root.join("target");
    let rel = ["compiled", "run"]
        .iter()
        .find_map(|dir| compiled.strip_prefix(target.join(dir)).ok())?;
    let mut parts = rel.components();
    let package = parts.next()?.as_os_str().to_string_lossy().into_owned();
    let rest = parts.as_path();
    if rest.as_os_str().is_empty() {
        return None;
    }
    Some(if package == project {
        root.join(rest)
    } else {
        root.join("dbt_packages").join(package).join(rest)
    })
}

/// Lines shown at most in a hover.
const MAX_SHOWN: u32 = 12;

/// The hover of a compiled line: where it came from, as a link, and the
/// source lines.
pub fn compiled_hover(
    lines: SourceLines,
    source_text: &str,
    source_uri: &str,
    shown_path: &str,
) -> String {
    let (start, end) = (lines.start, lines.end.min(lines.start + MAX_SHOWN - 1));
    let place = if lines.start == lines.end {
        format!("line {}", lines.start + 1)
    } else {
        format!("lines {}–{}", lines.start + 1, lines.end + 1)
    };
    let how = if lines.exact { "" } else { " (rendered)" };
    let body: Vec<&str> = source_text
        .lines()
        .skip(start as usize)
        .take((end - start + 1) as usize)
        .collect();
    let more = if end < lines.end { "\n…" } else { "" };
    format!(
        "**Compiled from** [`{shown_path}` {place}]({source_uri}#L{}){how}\n\n```jinja\n{}{more}\n```",
        lines.start + 1,
        body.join("\n")
    )
}

/// Definition on a compiled line: the first source line it came from.
pub fn compiled_definition(lines: SourceLines, source_uri: &str) -> Location {
    Location {
        uri: source_uri.to_string(),
        range: Range::zero_width(Position {
            line: lines.start,
            character: 0,
        }),
    }
}

#[cfg(test)]
#[path = "compiled_map_tests.rs"]
mod tests;
