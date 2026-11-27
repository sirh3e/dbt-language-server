//! References, symbols and highlights (FEATURE_REQUESTS P1/P3/P4).

use cogitator_lsp::{symbol_kind, DocumentHighlight, Location, Position, Range, SymbolInformation};
use cogitator_parser::TokenKind;

use super::{file_uri, model_name_from_uri, token_range, DbtContext, Document};

/// textDocument/references (P1): incoming `Ref` edges of the model under the
/// cursor (or of the open file's model).
pub fn references(
    ctx: &DbtContext,
    doc: Option<&Document>,
    uri: &str,
    pos: Position,
) -> Vec<Location> {
    let read = |p: &std::path::Path| std::fs::read_to_string(p).ok();
    if let Some(found) = super::var_references(ctx, doc, uri, pos, &read) {
        return found;
    }
    let model = doc
        .and_then(|d| {
            d.parsed
                .find_token_at_cursor(pos.line, pos.character)
                .and_then(|idx| {
                    let t = &d.parsed.tokens[idx];
                    (t.kind == TokenKind::Ref).then(|| d.parsed.literal(t).to_string())
                })
        })
        .or_else(|| model_name_from_uri(uri));
    let Some(model) = model else {
        return Vec::new();
    };

    // every ref('model') call site; a dependent whose file names none we
    // can find (unparsable, generated) still shows, as its whole file
    let sites = super::rename::model_ref_sites(ctx, &model, &read);
    let mut out: Vec<Location> = sites
        .iter()
        .flat_map(|(path, ranges)| {
            ranges.iter().map(|range| Location {
                uri: file_uri(path),
                range: *range,
            })
        })
        .collect();
    for node in ctx.graph.downstream_of(&model) {
        if !node.uri.as_os_str().is_empty() && !sites.contains_key(&node.uri) {
            out.push(Location {
                uri: file_uri(&node.uri),
                range: Range::default(),
            });
        }
    }
    out
}

/// textDocument/documentSymbol (flat): CTEs and macros defined in the
/// file, and the `ref()`/`source()` calls it makes, by position.
pub fn document_symbols(doc: &Document, uri: &str) -> Vec<SymbolInformation> {
    let local = |name: String, kind: i32, range: Range| SymbolInformation {
        name,
        kind,
        location: Location {
            uri: uri.to_string(),
            range,
        },
        container_name: None,
    };
    let mut symbols: Vec<SymbolInformation> = doc
        .parsed
        .def_tokens
        .iter()
        .map(|(name, token)| local(name.to_string(), symbol_kind::STRUCT, token_range(token)))
        .collect();
    for raw in cogitator_parser::jinja_scan::extract_macros(&doc.text) {
        let range = Range {
            start: Position {
                line: raw.start.0,
                character: raw.start.1,
            },
            end: Position {
                line: raw.end.0,
                character: raw.end.1,
            },
        };
        symbols.push(local(raw.name, symbol_kind::FUNCTION, range));
    }
    symbols.extend(
        call_symbols(doc)
            .into_iter()
            .map(|(name, range)| local(name, symbol_kind::PACKAGE, range)),
    );
    symbols.sort_by(|a, b| {
        let key = |s: &SymbolInformation| {
            (
                s.location.range.start.line,
                s.location.range.start.character,
            )
        };
        key(a).cmp(&key(b)).then_with(|| a.name.cmp(&b.name))
    });
    symbols
}

/// The file's `ref('m')` and `source('s', 't')` calls, named as written
/// (`ref(m)`, `source(s, t)`), at the name argument(s).
fn call_symbols(doc: &Document) -> Vec<(String, Range)> {
    let parsed = &doc.parsed;
    parsed
        .tokens
        .iter()
        .enumerate()
        .filter_map(|(idx, t)| match t.kind {
            TokenKind::Ref => Some((format!("ref({})", parsed.literal(t)), token_range(t))),
            TokenKind::SourceTable => {
                let source = parsed.lookback_find(idx, TokenKind::SourceName, 4)?;
                let range = Range {
                    start: token_range(source).start,
                    end: token_range(t).end,
                };
                let name = format!("source({}, {})", parsed.literal(source), parsed.literal(t));
                Some((name, range))
            }
            _ => None,
        })
        .collect()
}

/// textDocument/documentHighlight: occurrences of the same literal + kind.
pub fn document_highlights(doc: &Document, pos: Position) -> Vec<DocumentHighlight> {
    let Some(idx) = doc.parsed.find_token_at_cursor(pos.line, pos.character) else {
        return Vec::new();
    };
    let target = &doc.parsed.tokens[idx];
    super::same_literal_ranges(doc, target)
        .into_iter()
        .map(|range| DocumentHighlight { range, kind: 1 })
        .collect()
}
