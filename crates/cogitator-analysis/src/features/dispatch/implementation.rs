//! textDocument/implementation for dispatched macros, and the hover line
//! naming the implementation the active adapter picks. The cursor may
//! sit on an `adapter.dispatch('x', …)` call, on a call of a macro whose
//! body is such a dispatch, or on that macro's own definition header.

use cogitator_lsp::{Location, Position};
use cogitator_parser::macro_body::{dispatch_sites, DispatchCall};
use cogitator_parser::{LineTable, TokenKind};

use super::{adapter_prefixes, implementations, search_packages};
use crate::features::references::{files, occur};
use crate::features::{file_uri, DbtContext, Document};

/// The dispatch the cursor is about, if any.
pub fn dispatch_at_cursor(ctx: &DbtContext, doc: &Document, pos: Position) -> Option<DispatchCall> {
    let offset = LineTable::new(&doc.text)
        .line_start(pos.line as usize)
        .map(|s| s + pos.character)?;
    if let Some(site) = dispatch_sites(&doc.text)
        .into_iter()
        .find(|s| s.start <= offset && offset < s.end)
    {
        return Some(site.call);
    }
    if let Some(raw) = occur::macro_def_at(&doc.text, pos) {
        return raw.body.dispatch;
    }
    let parsed = &doc.parsed;
    let idx = parsed.find_token_at_cursor(pos.line, pos.character)?;
    let token = &parsed.tokens[idx];
    if token.kind != TokenKind::Macro {
        return None;
    }
    let qualifier = parsed
        .lookback_find(idx, TokenKind::Package, 2)
        .map(|t| parsed.literal(t));
    let from = calling_project(ctx, &doc.uri);
    files::resolve_macro(ctx, &from, qualifier, parsed.literal(token))?
        .body
        .dispatch
        .clone()
}

/// The project whose file the document is (a package's macro file, else
/// the root project).
pub(crate) fn calling_project(ctx: &DbtContext, uri: &str) -> String {
    let path = cogitator_lsp::uri_to_fs_path(uri).unwrap_or_else(|| uri.to_string());
    files::project_of_path(ctx, std::path::Path::new(&path))
}

/// Every implementation of the dispatch under the cursor: those dbt
/// would consider first (search order, then the active adapter's prefix
/// order), then every other `<adapter>__x` that is scanned.
pub fn implementation(ctx: &DbtContext, doc: &Document, pos: Position) -> Vec<Location> {
    let Some(call) = dispatch_at_cursor(ctx, doc, pos) else {
        return Vec::new();
    };
    ordered_implementations(ctx, &call)
        .into_iter()
        .map(|i| Location {
            uri: file_uri(&i.details.uri),
            range: i.details.range,
        })
        .collect()
}

/// [`implementations`] in dispatch order: search-order package index,
/// then prefix index for the active adapter; others after, by name.
pub(crate) fn ordered_implementations<'a>(
    ctx: &'a DbtContext,
    call: &DispatchCall,
) -> Vec<super::Implementation<'a>> {
    let packages = search_packages(ctx, call.namespace.as_deref());
    let prefixes = adapter_prefixes(ctx.dialect.as_str());
    let mut all = implementations(ctx, &call.name);
    let rank = |i: &super::Implementation<'_>| {
        (
            packages
                .iter()
                .position(|p| p == i.package)
                .unwrap_or(usize::MAX),
            prefixes
                .iter()
                .position(|p| *p == i.prefix)
                .unwrap_or(usize::MAX),
        )
    };
    all.sort_by_key(|i| (rank(i), i.package.to_string(), i.details.name.clone()));
    all
}

/// The hover text with the active-implementation line appended, when
/// the cursor is about a dispatch.
pub fn with_dispatch_line(ctx: &DbtContext, doc: &Document, pos: Position, text: String) -> String {
    let Some(call) = dispatch_at_cursor(ctx, doc, pos) else {
        return text;
    };
    let line = super::active_line(ctx, &call);
    if text.is_empty() {
        line
    } else {
        format!("{text}\n\n{line}")
    }
}
