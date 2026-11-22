//! Macro call hierarchy (03 §9 addendum): prepare answers one
//! Function-kind item for the macro at a call site or at its definition
//! header; incomingCalls lists the callers — models, tests, analyses
//! (the graph's MacroCall edges) and macros (every scanned macro file) —
//! with the exact call ranges; outgoingCalls lists the macros a macro
//! (or a model item) calls. Calls resolve as dbt resolves them, so
//! `pkg.name()` and a bare `name()` of another package's macro stay
//! apart. Files are re-read from disk (the rename.rs precedent).

use std::path::Path;

use cogitator_lsp::{
    symbol_kind, CallHierarchyIncomingCall, CallHierarchyItem, CallHierarchyOutgoingCall, Position,
};
use cogitator_parser::{jinja_scan, DocumentParser, JinjaSqlParser};

use super::references::files::{path_of, project_of_path, resolve_macro, Reader};
use super::references::macros::{call_sites, definition_range};
use super::references::occur::{macro_body, macro_calls};
use super::references::target::{target_at, Target};
use super::{file_uri, token_range, DbtContext, Document};

/// The macro under the cursor as a call-hierarchy item: a call site or a
/// definition header, resolved in the scanned packages.
pub fn prepare_macro_hierarchy(
    ctx: &DbtContext,
    doc: &Document,
    pos: Position,
) -> Option<CallHierarchyItem> {
    prepare_call_hierarchy(ctx, doc, "", pos)
}

/// [`prepare_macro_hierarchy`] knowing the document's uri, which tells a
/// package's macro file (and its bare calls) from the root project's.
pub fn prepare_call_hierarchy(
    ctx: &DbtContext,
    doc: &Document,
    uri: &str,
    pos: Position,
) -> Option<CallHierarchyItem> {
    match target_at(ctx, doc, uri, pos)? {
        Target::Macro { package, name } => macro_item(ctx, &package, &name),
        _ => None,
    }
}

fn macro_item(ctx: &DbtContext, package: &str, name: &str) -> Option<CallHierarchyItem> {
    let (path, selection) = definition_range(ctx, package, name)?;
    let details = ctx.macro_detail_map.get(package)?.get(name)?;
    Some(CallHierarchyItem {
        name: name.to_string(),
        kind: symbol_kind::FUNCTION,
        uri: file_uri(&path),
        range: details.range,
        selection_range: selection,
    })
}

/// Every caller of `item`'s macro, each with the exact call ranges.
pub fn macro_incoming_calls(
    ctx: &DbtContext,
    item: &CallHierarchyItem,
) -> Vec<CallHierarchyIncomingCall> {
    let reader = Reader::disk(None);
    let Some(package) = item_package(ctx, item) else {
        return Vec::new();
    };
    let mut sites = call_sites(ctx, &reader, &package, &item.name);
    sites.sort_by(|a, b| {
        (a.caller_is_macro, &a.caller, &a.path).cmp(&(b.caller_is_macro, &b.caller, &b.path))
    });
    sites
        .into_iter()
        .map(|site| CallHierarchyIncomingCall {
            from: CallHierarchyItem {
                name: site.caller,
                // models render as classes, macros as functions
                kind: if site.caller_is_macro {
                    symbol_kind::FUNCTION
                } else {
                    symbol_kind::CLASS
                },
                uri: file_uri(&site.path),
                range: site.caller_range,
                selection_range: site.caller_range,
            },
            from_ranges: site.ranges,
        })
        .collect()
}

/// The macros `item` calls: a macro item's body, or a model item's file.
pub fn macro_outgoing_calls(
    ctx: &DbtContext,
    item: &CallHierarchyItem,
) -> Vec<CallHierarchyOutgoingCall> {
    let Some(path) = path_of(&item.uri) else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    outgoing_in_text(ctx, item, &path, &text)
}

/// [`macro_outgoing_calls`] over a given file text.
pub fn outgoing_in_text(
    ctx: &DbtContext,
    item: &CallHierarchyItem,
    path: &Path,
    text: &str,
) -> Vec<CallHierarchyOutgoingCall> {
    let Ok(parsed) = JinjaSqlParser.parse(text, &ctx.dialect) else {
        return Vec::new();
    };
    let window = if item.kind == symbol_kind::FUNCTION {
        let macros = jinja_scan::extract_macros(text);
        match macro_body(&parsed, &macros, &item.name) {
            Some(window) => window,
            None => return Vec::new(),
        }
    } else {
        0..parsed.tokens.len()
    };
    let project = project_of_path(ctx, path);
    let mut out: Vec<CallHierarchyOutgoingCall> = Vec::new();
    for (idx, qualifier, name) in macro_calls(&parsed) {
        if !window.contains(&idx) {
            continue;
        }
        let Some(callee) = resolve_macro(ctx, &project, qualifier, name)
            .and_then(|d| macro_item(ctx, &d.project_name, name))
        else {
            continue;
        };
        let range = token_range(&parsed.tokens[idx]);
        match out
            .iter_mut()
            .find(|c| c.to.uri == callee.uri && c.to.name == callee.name)
        {
            Some(call) => call.from_ranges.push(range),
            None => out.push(CallHierarchyOutgoingCall {
                to: callee,
                from_ranges: vec![range],
            }),
        }
    }
    out.sort_by(|a, b| a.to.name.cmp(&b.to.name));
    out
}

/// The package of a macro item: the project its file belongs to, checked
/// to define the macro (the item round-trips through the client).
fn item_package(ctx: &DbtContext, item: &CallHierarchyItem) -> Option<String> {
    let path = path_of(&item.uri)?;
    let project = project_of_path(ctx, &path);
    if ctx
        .macro_detail_map
        .get(&project)
        .is_some_and(|m| m.contains_key(&item.name))
    {
        return Some(project);
    }
    resolve_macro(ctx, &ctx.project.name, None, &item.name).map(|d| d.project_name.clone())
}
