//! Macro call hierarchy (03 §9 addendum): prepare answers one
//! Function-kind item for the macro under the cursor; incomingCalls
//! walks the graph's MacroCall edges and locates the exact call
//! tokens in each caller (files re-read from disk, the rename.rs
//! precedent for dependents).

use cogitator_lsp::{symbol_kind, CallHierarchyIncomingCall, CallHierarchyItem, Position, Range};
use cogitator_parser::{DocumentParser, JinjaSqlParser, TokenKind};

use super::{file_uri, token_range, DbtContext, Document};

/// The macro under the cursor as a call-hierarchy item: the cursor
/// must sit on a macro-call token, and the macro must resolve in some
/// scanned package (own project first).
pub fn prepare_macro_hierarchy(
    ctx: &DbtContext,
    doc: &Document,
    pos: Position,
) -> Option<CallHierarchyItem> {
    let idx = doc.parsed.find_token_at_cursor(pos.line, pos.character)?;
    let token = &doc.parsed.tokens[idx];
    if token.kind != TokenKind::Macro {
        return None;
    }
    let name = doc.parsed.literal(token);
    let details = lookup_macro(ctx, name)?;
    Some(CallHierarchyItem {
        name: name.to_string(),
        kind: symbol_kind::FUNCTION,
        uri: file_uri(&details.uri),
        range: details.range,
        selection_range: details.range,
    })
}

/// Every model calling `item`'s macro, each with the exact call-token
/// ranges inside its file.
pub fn macro_incoming_calls(
    ctx: &DbtContext,
    item: &CallHierarchyItem,
) -> Vec<CallHierarchyIncomingCall> {
    let mut out = Vec::new();
    let mut callers: Vec<_> = ctx
        .graph
        .macro_call_sites(&item.name)
        .filter(|node| !node.uri.as_os_str().is_empty())
        .collect();
    callers.sort_by(|a, b| a.name.cmp(&b.name));
    callers.dedup_by(|a, b| a.uri == b.uri);
    for caller in callers {
        let Ok(text) = std::fs::read_to_string(&caller.uri) else {
            continue;
        };
        let Ok(parsed) = JinjaSqlParser.parse(&text, &ctx.dialect) else {
            continue;
        };
        let from_ranges: Vec<Range> = parsed
            .tokens
            .iter()
            .filter(|t| t.kind == TokenKind::Macro && parsed.literal(t) == item.name)
            .map(token_range)
            .collect();
        if from_ranges.is_empty() {
            continue; // stale edge: the call is gone from the file
        }
        out.push(CallHierarchyIncomingCall {
            from: CallHierarchyItem {
                name: caller.name.clone(),
                kind: symbol_kind::CLASS, // models render as types/classes
                uri: file_uri(&caller.uri),
                range: Range::default(),
                selection_range: Range::default(),
            },
            from_ranges,
        });
    }
    out
}

/// A macro by bare name across the scanned packages, own project
/// winning on collision (03 §5 resolution order).
fn lookup_macro<'a>(ctx: &'a DbtContext, name: &str) -> Option<&'a crate::context::MacroDetails> {
    let own = ctx.project.name.as_str();
    ctx.macro_detail_map
        .get(own)
        .and_then(|macros| macros.get(name))
        .or_else(|| {
            ctx.macro_detail_map
                .values()
                .find_map(|macros| macros.get(name))
        })
}
