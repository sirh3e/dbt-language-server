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

/// textDocument/documentSymbol (flat): CTEs + macros defined in the file.
pub fn document_symbols(doc: &Document, uri: &str) -> Vec<SymbolInformation> {
    let mut symbols: Vec<SymbolInformation> = doc
        .parsed
        .def_tokens
        .iter()
        .map(|(name, token)| SymbolInformation {
            name: name.to_string(),
            kind: symbol_kind::STRUCT,
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

/// workspace/symbol: models, sources and macros matching `query`. The
/// `tag:` and `selector:` prefixes go through the selector engine over a
/// universe built from the context alone (no file reads); the server
/// uses [`workspace_symbols_in`] with the project's full universe.
pub fn workspace_symbols(ctx: &DbtContext, query: &str) -> Vec<SymbolInformation> {
    let universe = crate::selectors::universe(ctx, &|_| None);
    workspace_symbols_in(ctx, &universe, query)
}

/// [`workspace_symbols`] over a given selector universe.
pub fn workspace_symbols_in(
    ctx: &DbtContext,
    universe: &crate::selectors::Universe,
    query: &str,
) -> Vec<SymbolInformation> {
    use crate::selectors::{self, glob, Indirect, ResourceType};
    let raw = query;
    let query = query.to_lowercase();

    // `tag:x` → nodes carrying a tag that contains x (a glob as written);
    // `selector:x` → selectors.yml entries. A bare prefix lists
    // everything of that kind.
    if let Some(tag) = raw.strip_prefix("tag:") {
        // tags compare as dbt does: case-sensitive
        let tag = tag.trim();
        let pattern = if glob::is_glob(tag) {
            tag.to_string()
        } else {
            format!("*{tag}*")
        };
        let Ok(eval) = selectors::select(universe, &[], &format!("tag:{pattern}"), Indirect::Empty)
        else {
            return Vec::new();
        };
        let mut out: Vec<SymbolInformation> = selectors::sorted(universe, &eval)
            .into_iter()
            .map(|n| SymbolInformation {
                name: n.display_name(),
                kind: match n.resource_type {
                    ResourceType::Source => symbol_kind::INTERFACE,
                    ResourceType::Test | ResourceType::UnitTest => symbol_kind::FUNCTION,
                    _ => symbol_kind::CLASS,
                },
                location: Location {
                    uri: file_uri(&n.uri),
                    range: n.range,
                },
            })
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        return out;
    }
    if let Some(selector) = query.strip_prefix("selector:") {
        let selector = selector.trim();
        let mut out: Vec<SymbolInformation> = ctx
            .selectors
            .definitions
            .iter()
            .filter(|d| selector.is_empty() || d.name.to_lowercase().contains(selector))
            .filter_map(|d| ctx.selector_detail_map.get(&d.name))
            .map(|s| SymbolInformation {
                name: s.name.clone(),
                kind: symbol_kind::NAMESPACE,
                location: Location {
                    uri: file_uri(&s.uri),
                    range: s.range,
                },
            })
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out.dedup_by(|a, b| a.name == b.name);
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
                kind: symbol_kind::MODULE,
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
                kind: symbol_kind::NAMESPACE,
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
    dedup_same_file(&mut out);
    out
}

/// One symbol per name and file: a package installed by a link, or a
/// folder of linked macros, reaches the same file under several paths.
fn dedup_same_file(out: &mut Vec<SymbolInformation>) {
    let mut seen = std::collections::HashSet::new();
    out.retain(|s| {
        let path = cogitator_lsp::uri_to_fs_path(&s.location.uri)
            .map(std::path::PathBuf::from)
            .map(|p| std::fs::canonicalize(&p).unwrap_or(p));
        seen.insert((s.name.clone(), s.kind, path, s.location.range.start.line))
    });
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
