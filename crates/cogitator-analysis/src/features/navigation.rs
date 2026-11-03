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

    ctx.graph
        .downstream_of(&model)
        .filter(|node| !node.uri.as_os_str().is_empty())
        .map(|node| Location {
            uri: file_uri(&node.uri),
            range: Range::default(),
        })
        .collect()
}

/// textDocument/documentSymbol (flat): CTEs + macros defined in the file.
pub fn document_symbols(doc: &Document, uri: &str) -> Vec<SymbolInformation> {
    let mut symbols: Vec<SymbolInformation> = doc
        .parsed
        .def_tokens
        .iter()
        .map(|(name, token)| SymbolInformation {
            name: name.to_string(),
            kind: symbol_kind::VARIABLE,
            location: Location {
                uri: uri.to_string(),
                range: token_range(token),
            },
        })
        .collect();
    for raw in cogitator_parser::jinja_scan::extract_macros(&doc.text) {
        symbols.push(SymbolInformation {
            name: raw.name,
            kind: symbol_kind::FUNCTION,
            location: Location {
                uri: uri.to_string(),
                range: Range {
                    start: Position {
                        line: raw.start.0,
                        character: raw.start.1,
                    },
                    end: Position {
                        line: raw.end.0,
                        character: raw.end.1,
                    },
                },
            },
        });
    }
    symbols.sort_by(|a, b| {
        (
            a.location.range.start.line,
            a.location.range.start.character,
            a.name.clone(),
        )
            .cmp(&(
                b.location.range.start.line,
                b.location.range.start.character,
                b.name.clone(),
            ))
    });
    symbols
}

/// workspace/symbol: models, sources and macros matching `query`.
pub fn workspace_symbols(ctx: &DbtContext, query: &str) -> Vec<SymbolInformation> {
    let query = query.to_lowercase();

    // `tag:x` → models carrying tag x; `selector:x` → selectors.yml
    // entries. A bare prefix lists everything of that kind.
    if let Some(tag) = query.strip_prefix("tag:") {
        let tag = tag.trim();
        let mut out: Vec<SymbolInformation> = ctx
            .model_detail_map
            .iter()
            .filter(|(_, m)| {
                tag.is_empty() || m.tags.iter().any(|t| t.to_lowercase().contains(tag))
            })
            .filter(|(_, m)| !m.tags.is_empty())
            .map(|(name, m)| SymbolInformation {
                name: name.clone(),
                kind: symbol_kind::CLASS,
                location: Location {
                    uri: file_uri(&m.uri),
                    range: Range::default(),
                },
            })
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        return out;
    }
    if let Some(selector) = query.strip_prefix("selector:") {
        let selector = selector.trim();
        let mut out: Vec<SymbolInformation> = ctx
            .selector_detail_map
            .iter()
            .filter(|(name, _)| selector.is_empty() || name.to_lowercase().contains(selector))
            .map(|(name, s)| SymbolInformation {
                name: name.clone(),
                kind: symbol_kind::KEY,
                location: Location {
                    uri: file_uri(&s.uri),
                    range: s.range,
                },
            })
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        return out;
    }

    if let Some(exposure) = query.strip_prefix("exposure:") {
        let exposure = exposure.trim();
        let mut out: Vec<SymbolInformation> = ctx
            .exposure_detail_map
            .iter()
            .chain(&ctx.metric_detail_map)
            .chain(&ctx.semantic_model_detail_map)
            .filter(|(name, _)| exposure.is_empty() || name.to_lowercase().contains(exposure))
            .map(|(name, c)| SymbolInformation {
                name: name.clone(),
                kind: symbol_kind::EVENT,
                location: Location {
                    uri: file_uri(&c.uri),
                    range: c.range,
                },
            })
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        return out;
    }

    let matches = |name: &str| query.is_empty() || name.to_lowercase().contains(&query);
    let mut out = Vec::new();
    for (name, s) in &ctx.selector_detail_map {
        if matches(name) {
            out.push(SymbolInformation {
                name: name.clone(),
                kind: symbol_kind::KEY,
                location: Location {
                    uri: file_uri(&s.uri),
                    range: s.range,
                },
            });
        }
    }
    for (name, m) in &ctx.model_detail_map {
        if matches(name) {
            out.push(SymbolInformation {
                name: name.clone(),
                kind: symbol_kind::CLASS,
                location: Location {
                    uri: file_uri(&m.uri),
                    range: Range::default(),
                },
            });
        }
    }
    for (name, s) in &ctx.source_detail_map {
        if matches(name) {
            out.push(SymbolInformation {
                name: name.clone(),
                kind: symbol_kind::INTERFACE,
                location: Location {
                    uri: file_uri(&s.uri),
                    range: s.range,
                },
            });
        }
    }
    for macros in ctx.macro_detail_map.values() {
        for (name, m) in macros {
            if matches(name) {
                out.push(SymbolInformation {
                    name: name.clone(),
                    kind: symbol_kind::FUNCTION,
                    location: Location {
                        uri: file_uri(&m.uri),
                        range: m.range,
                    },
                });
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// textDocument/documentHighlight: occurrences of the same literal + kind.
pub fn document_highlights(doc: &Document, pos: Position) -> Vec<DocumentHighlight> {
    let Some(idx) = doc.parsed.find_token_at_cursor(pos.line, pos.character) else {
        return Vec::new();
    };
    let target = &doc.parsed.tokens[idx];
    let literal = doc.parsed.literal(target);
    doc.parsed
        .tokens
        .iter()
        .filter(|t| t.kind == target.kind && doc.parsed.literal(t) == literal)
        .map(|t| DocumentHighlight {
            range: token_range(t),
            kind: 1,
        })
        .collect()
}
