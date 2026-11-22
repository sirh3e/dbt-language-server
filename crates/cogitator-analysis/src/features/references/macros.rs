//! Macro call sites project-wide: the graph's MacroCall callers (models,
//! tests, analyses) plus every scanned macro file, each call resolved as
//! dbt would (qualified or bare) so only calls of THIS macro count.
//! Shared by references, rename and the call hierarchy.

use std::path::PathBuf;

use cogitator_lsp::Range;
use cogitator_parser::jinja_scan;
use cogitator_parser::{DocumentParser, JinjaSqlParser};

use super::super::{token_range, DbtContext};
use super::files::{macro_files, node_files, resolve_macro, Reader};
use super::occur::{enclosing_macro, macro_calls, span};

/// The calls of one caller in one file.
#[derive(Debug, Clone)]
pub(crate) struct CallSite {
    pub path: PathBuf,
    /// The calling model/test/analysis, or the calling macro.
    pub caller: String,
    pub caller_is_macro: bool,
    /// For a macro caller: its definition header's name.
    pub caller_range: Range,
    pub ranges: Vec<Range>,
}

/// Every call of `package`'s macro `name`.
pub(crate) fn call_sites(
    ctx: &DbtContext,
    reader: &Reader<'_>,
    package: &str,
    name: &str,
) -> Vec<CallSite> {
    let mut out = Vec::new();
    let callers: Vec<_> = ctx.graph.macro_call_sites(name).collect();
    for path in node_files(callers.iter().copied()) {
        let caller = callers
            .iter()
            .find(|n| n.uri == path)
            .map(|n| n.name.clone())
            .unwrap_or_default();
        let ranges = calls_in(ctx, reader, &path, &ctx.project.name, package, name)
            .into_iter()
            .map(|(_, r)| r)
            .collect::<Vec<_>>();
        if !ranges.is_empty() {
            out.push(CallSite {
                path,
                caller,
                caller_is_macro: false,
                caller_range: Range::default(),
                ranges,
            });
        }
    }
    for (path, project) in macro_files(ctx) {
        if out.iter().any(|s| s.path == path) {
            continue;
        }
        let Some(text) = reader.read(&path) else {
            continue;
        };
        let macros = jinja_scan::extract_macros(&text);
        let mut by_macro: Vec<CallSite> = Vec::new();
        for (at, range) in calls_in_text(ctx, &text, &project, package, name) {
            let Some(enclosing) = enclosing_macro(&macros, at) else {
                continue;
            };
            match by_macro.iter_mut().find(|s| s.caller == enclosing.name) {
                Some(site) => site.ranges.push(range),
                None => by_macro.push(CallSite {
                    path: path.clone(),
                    caller: enclosing.name.clone(),
                    caller_is_macro: true,
                    caller_range: span(
                        enclosing.start.0,
                        enclosing.start.1,
                        enclosing.name.len() as u32,
                    ),
                    ranges: vec![range],
                }),
            }
        }
        out.extend(by_macro);
    }
    out
}

fn calls_in(
    ctx: &DbtContext,
    reader: &Reader<'_>,
    path: &std::path::Path,
    project: &str,
    package: &str,
    name: &str,
) -> Vec<((u32, u32), Range)> {
    reader
        .read(path)
        .map(|text| calls_in_text(ctx, &text, project, package, name))
        .unwrap_or_default()
}

/// The calls in `text` (a file of `project`) that resolve to the macro.
pub(crate) fn calls_in_text(
    ctx: &DbtContext,
    text: &str,
    project: &str,
    package: &str,
    name: &str,
) -> Vec<((u32, u32), Range)> {
    let Ok(parsed) = JinjaSqlParser.parse(text, &ctx.dialect) else {
        return Vec::new();
    };
    macro_calls(&parsed)
        .into_iter()
        .filter(|(_, q, n)| {
            *n == name
                && resolve_macro(ctx, project, *q, n).is_some_and(|d| d.project_name == package)
        })
        .map(|(i, _, _)| {
            let t = &parsed.tokens[i];
            ((t.line, t.column), token_range(t))
        })
        .collect()
}

/// The definition header's name range of `package`'s macro `name`.
pub(crate) fn definition_range(
    ctx: &DbtContext,
    package: &str,
    name: &str,
) -> Option<(PathBuf, Range)> {
    let details = ctx.macro_detail_map.get(package)?.get(name)?;
    let start = details.range.start;
    Some((
        details.uri.clone(),
        span(start.line, start.character, name.len() as u32),
    ))
}
