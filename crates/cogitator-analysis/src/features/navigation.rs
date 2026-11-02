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
    let matches = |name: &str| query.is_empty() || name.to_lowercase().contains(&query);
    let mut out = Vec::new();
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

/// textDocument/codeLens (FEATURE_REQUESTS lens batch): model files get
/// upstream/downstream counts plus entry points for schema, lineage and
/// per-model dbt execution; macro files get per-macro call-site counts.
pub fn code_lenses(ctx: &DbtContext, uri: &str) -> Vec<cogitator_lsp::CodeLens> {
    let mut lenses = model_lenses(ctx, uri);
    lenses.extend(macro_lenses(ctx, uri));
    lenses
}

fn model_lenses(ctx: &DbtContext, uri: &str) -> Vec<cogitator_lsp::CodeLens> {
    let Some(model) = model_name_from_uri(uri) else {
        return Vec::new();
    };
    if !ctx.model_detail_map.contains_key(&model) {
        return Vec::new();
    }
    let downstream = ctx.graph.downstream_of(&model).count();
    let upstream = ctx.graph.upstream_of(&model).count();
    let arg = serde_json::json!({ "uri": uri });

    let display = |title: String| lens(title, "", Vec::new());
    let command =
        |title: &str, command: &'static str| lens(title.to_string(), command, vec![arg.clone()]);

    vec![
        display(plural(downstream, "downstream")),
        display(plural(upstream, "upstream")),
        command("Go to schema", "dbt.openSchema"),
        command("Column lineage", "dbt.showLineage"),
        command("dbt run", "dbt.run"),
        command("dbt test", "dbt.test"),
        command("dbt compile", "dbt.compile"),
    ]
}

/// One "N call sites" lens per macro defined in this file, anchored at the
/// macro's signature.
fn macro_lenses(ctx: &DbtContext, uri: &str) -> Vec<cogitator_lsp::CodeLens> {
    let path = cogitator_lsp::uri_to_fs_path(uri).unwrap_or_else(|| uri.to_string());
    let mut lenses: Vec<cogitator_lsp::CodeLens> = ctx
        .macro_detail_map
        .values()
        .flat_map(|macros| macros.values())
        // Path equality compares components, so `/` and `\` separators match
        .filter(|m| m.uri.as_path() == std::path::Path::new(path.as_str()))
        .map(|m| {
            let count = ctx.graph.macro_call_sites(&m.name).count();
            cogitator_lsp::CodeLens {
                range: m.range,
                command: cogitator_lsp::Command {
                    title: plural(count, "call site"),
                    command: String::new(),
                    arguments: Vec::new(),
                },
            }
        })
        .collect();
    lenses.sort_by_key(|l| (l.range.start.line, l.range.start.character));
    lenses
}

fn plural(n: usize, noun: &str) -> String {
    match n {
        1 => format!("1 {noun}"),
        n => format!("{n} {noun}s"),
    }
}

fn lens(
    title: String,
    command: &str,
    arguments: Vec<serde_json::Value>,
) -> cogitator_lsp::CodeLens {
    cogitator_lsp::CodeLens {
        range: Range::default(),
        command: cogitator_lsp::Command {
            title,
            command: command.to_string(),
            arguments,
        },
    }
}
