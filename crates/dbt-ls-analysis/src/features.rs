//! Feature algorithms (specs/rust-port/03 §9): pure functions over the
//! context snapshot and a parsed document; the dispatcher serializes.

use std::path::Path;
use std::sync::LazyLock;

use dbt_ls_lsp::{completion_kind, CompletionItem, Location, Position, Range};
use dbt_ls_parser::TokenKind;
use regex::Regex;

use crate::context::DbtContext;
use crate::Document;

fn file_uri(path: &Path) -> String {
    format!("file://{}", path.display())
}

/// Hover (03 §9.1). Default: empty contents.
pub fn hover(ctx: &DbtContext, doc: &Document, pos: Position) -> String {
    let Some(idx) = doc.parsed.find_token_at_cursor(pos.line, pos.character) else {
        return String::new();
    };
    let parsed = &doc.parsed;
    let token = &parsed.tokens[idx];
    let literal = parsed.literal(token);

    match token.kind {
        TokenKind::Ref => ctx
            .model_detail_map
            .get(literal)
            .map(|m| m.description.clone())
            .unwrap_or_default(),
        TokenKind::SourceName => ctx
            .source_detail_map
            .get(literal)
            .map(|s| s.description.clone())
            .unwrap_or_default(),
        TokenKind::SourceTable => {
            let Some(source_token) = parsed.lookback_find(idx, TokenKind::SourceName, 4) else {
                return String::new();
            };
            let source_name = parsed.literal(source_token);
            let Some(source) = ctx.source_detail_map.get(source_name) else {
                return String::new();
            };
            let table = source.tables.get(literal);
            format!(
                "Source: {}\n{}\n\nTable: {}\n{}",
                source.name,
                source.description,
                table.map(|t| t.name.as_str()).unwrap_or_default(),
                table.map(|t| t.description.as_str()).unwrap_or_default(),
            )
        }
        TokenKind::Var => {
            let value = ctx
                .variable_detail_map
                .get(literal)
                .map(|v| display_value(&v.value))
                .unwrap_or_default();
            format!("{literal}: {value}")
        }
        TokenKind::Macro => {
            let package = parsed
                .lookback_find(idx, TokenKind::Package, 2)
                .map(|t| parsed.literal(t).to_string())
                .unwrap_or_else(|| ctx.project.name.clone());
            ctx.macro_detail_map
                .get(&package)
                .and_then(|m| m.get(literal))
                .map(|m| m.description.clone())
                .unwrap_or_default()
        }
        _ => ctx
            .dialect
            .function_docs()
            .get(literal)
            .cloned()
            .unwrap_or_default(),
    }
}

/// Definition (03 §9.2). Default: zero-width range at the request cursor.
pub fn definition(ctx: &DbtContext, doc: &Document, uri: &str, pos: Position) -> Location {
    let fallback = Location {
        uri: uri.to_string(),
        range: Range::zero_width(pos),
    };
    let Some(idx) = doc.parsed.find_token_at_cursor(pos.line, pos.character) else {
        return fallback;
    };
    let parsed = &doc.parsed;
    let token = &parsed.tokens[idx];
    let literal = parsed.literal(token);

    match token.kind {
        TokenKind::Ref => ctx
            .model_detail_map
            .get(literal)
            .map(|m| Location {
                uri: file_uri(&m.uri),
                range: Range::default(),
            })
            .unwrap_or(fallback),
        TokenKind::SourceName => ctx
            .source_detail_map
            .get(literal)
            .filter(|s| !s.name.is_empty())
            .map(|s| Location {
                uri: file_uri(&s.uri),
                range: s.range,
            })
            .unwrap_or(fallback),
        TokenKind::SourceTable => parsed
            .lookback_find(idx, TokenKind::SourceName, 4)
            .and_then(|src| ctx.source_detail_map.get(parsed.literal(src)))
            .and_then(|s| s.tables.get(literal))
            .filter(|t| !t.name.is_empty())
            .map(|t| Location {
                uri: file_uri(&t.uri),
                range: t.range,
            })
            .unwrap_or(fallback),
        TokenKind::Var => ctx
            .variable_detail_map
            .get(literal)
            .map(|v| Location {
                uri: file_uri(&v.uri),
                range: v.range,
            })
            .unwrap_or(fallback),
        TokenKind::Macro => {
            let package = parsed
                .lookback_find(idx, TokenKind::Package, 2)
                .map(|t| parsed.literal(t).to_string())
                .unwrap_or_else(|| ctx.project.name.clone());
            ctx.macro_detail_map
                .get(&package)
                .and_then(|m| m.get(literal))
                .map(|m| Location {
                    uri: file_uri(&m.uri),
                    range: m.range,
                })
                .unwrap_or(fallback)
        }
        _ => {
            // CTE jump within the file (03 §9.2 default arm).
            parsed
                .def_tokens
                .get(literal)
                .map(|def| Location {
                    uri: uri.to_string(),
                    range: Range::zero_width(Position {
                        line: def.line,
                        character: def.column,
                    }),
                })
                .unwrap_or(fallback)
        }
    }
}

/// `dbt.goToSchema` (03 §9.3). `None` serializes as a null result.
pub fn go_to_schema(
    ctx: &DbtContext,
    doc: Option<&Document>,
    uri: &str,
    pos: Position,
) -> Option<Location> {
    if let Some(doc) = doc {
        if let Some(idx) = doc.parsed.find_token_at_cursor(pos.line, pos.character) {
            let token = &doc.parsed.tokens[idx];
            if token.kind == TokenKind::Ref {
                if let Some(location) = schema_location(ctx, doc.parsed.literal(token)) {
                    return Some(location);
                }
            }
        }
    }
    let model_name = model_name_from_uri(uri)?;
    schema_location(ctx, &model_name)
}

fn schema_location(ctx: &DbtContext, model: &str) -> Option<Location> {
    let details = ctx.model_detail_map.get(model)?;
    let schema_uri = details
        .schema_uri
        .as_ref()
        .filter(|p| !p.as_os_str().is_empty())?;
    Some(Location {
        uri: file_uri(schema_uri),
        range: details.schema_range,
    })
}

pub fn model_name_from_uri(uri: &str) -> Option<String> {
    let path = uri.strip_prefix("file://").unwrap_or(uri);
    let base = Path::new(path).file_name()?.to_string_lossy();
    base.strip_suffix(".sql").map(str::to_string)
}

// ---------------------------------------------------------------------------
// Completion (03 §9.4)
// ---------------------------------------------------------------------------

// Q1 fixed: `[a-zA-Z0-9_]` instead of Go's `[a-zA-z]` typo.
static REF_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\bref\(('|")[a-zA-Z0-9_]*$"#).unwrap());
static SOURCE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\bsource\(('|")[a-zA-Z0-9_]*$"#).unwrap());
static VAR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\bvar\(('|")[a-zA-Z0-9_]*$"#).unwrap());
static JINJA_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{\{\s*").unwrap());

pub fn completion(ctx: &DbtContext, doc: &Document, pos: Position) -> Vec<CompletionItem> {
    let lines: Vec<&str> = doc.text.split('\n').collect();
    let Some(&line) = lines.get(pos.line as usize) else {
        return Vec::new(); // Go panicked here (Q8-adjacent); degrade instead
    };
    let cursor = (pos.character as usize).min(line.len());
    let cursor = (0..=cursor)
        .rev()
        .find(|&i| line.is_char_boundary(i))
        .unwrap_or(0);
    let before = &line[..cursor];
    let after = &line[cursor..];

    if REF_RE.is_match(before) {
        ref_items(ctx, &suffix_for(line, after, "ref"))
    } else if SOURCE_RE.is_match(before) {
        source_items(ctx, &suffix_for(line, after, "source"), &quote_type(before))
    } else if VAR_RE.is_match(before) {
        var_items(ctx, &suffix_for(line, after, "var"))
    } else if JINJA_RE.is_match(before) {
        macro_items(ctx)
    } else {
        ctx.dialect.function_completion_items()
    }
}

fn ref_items(ctx: &DbtContext, suffix: &str) -> Vec<CompletionItem> {
    ctx.model_detail_map
        .iter()
        .map(|(name, m)| CompletionItem {
            label: name.clone(),
            detail: format!("Project: {}", m.project_name),
            documentation: String::new(), // resolved lazily (completionItem/resolve)
            kind: completion_kind::REFERENCE,
            insert_text: format!("{name}{suffix}"),
            sort_text: name.clone(),
            data: Some(serde_json::json!({"kind": "model", "name": name})),
        })
        .collect()
}

fn source_items(ctx: &DbtContext, suffix: &str, quote: &str) -> Vec<CompletionItem> {
    ctx.source_detail_map
        .iter()
        .flat_map(|(key, source)| {
            source.tables.values().map(move |table| CompletionItem {
                label: format!("{} - {}", source.name, table.name),
                detail: format!("Source: {}", source.name),
                documentation: String::new(),
                kind: completion_kind::REFERENCE,
                insert_text: format!("{}{quote}, {quote}{}{suffix}", source.name, table.name),
                sort_text: key.clone(),
                data: Some(
                    serde_json::json!({"kind": "source", "name": source.name, "table": table.name}),
                ),
            })
        })
        .collect()
}

fn var_items(ctx: &DbtContext, suffix: &str) -> Vec<CompletionItem> {
    ctx.variable_detail_map
        .keys()
        .map(|name| CompletionItem {
            label: name.clone(),
            detail: name.clone(),
            documentation: String::new(),
            kind: completion_kind::VARIABLE,
            insert_text: format!("{name}{suffix}"),
            sort_text: name.clone(),
            data: Some(serde_json::json!({"kind": "variable", "name": name})),
        })
        .collect()
}

fn macro_items(ctx: &DbtContext) -> Vec<CompletionItem> {
    ctx.macro_detail_map
        .values()
        .flat_map(|macros| macros.values())
        .map(|m| {
            let insert_text = if ctx.project.name == m.project_name {
                m.name.clone()
            } else {
                format!("{}.{}", m.project_name, m.name)
            };
            CompletionItem {
                label: m.name.clone(),
                detail: format!("Project: {}", m.project_name),
                documentation: String::new(),
                kind: completion_kind::SNIPPET,
                insert_text,
                sort_text: m.name.clone(),
                data: Some(serde_json::json!({
                    "kind": "macro", "name": m.name, "package": m.project_name,
                })),
            }
        })
        .collect()
}

/// Closing-suffix mirroring (03 §9.4.1): `{{ ref('` → `') }}`.
fn suffix_for(line: &str, after: &str, kind: &str) -> String {
    if !after.is_empty() {
        return String::new();
    }
    let re = Regex::new(&format!(r#"\{{\{{\s*{kind}\(('|")"#)).expect("static pattern");
    let Some(prefix) = re.find(line).map(|m| m.as_str()) else {
        return String::new();
    };
    let without_kind = prefix.replacen(kind, "", 1);
    without_kind
        .chars()
        .rev()
        .map(|c| match c {
            '(' => ')',
            '{' => '}',
            other => other,
        })
        .collect()
}

/// Quote style for the source item's middle quotes (03 §9.4.2; Q3 fixed:
/// genuinely the last quote before the cursor).
fn quote_type(before: &str) -> String {
    before
        .chars()
        .rev()
        .find(|c| *c == '\'' || *c == '"')
        .map(String::from)
        .unwrap_or_default()
}

pub(crate) fn display_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Post-parity features (FEATURE_REQUESTS.md)
// ---------------------------------------------------------------------------

use dbt_ls_lsp::{
    diagnostic_severity, symbol_kind, Diagnostic, DocumentHighlight, FoldingRange,
    ParameterInformation, SelectionRange, SemanticTokens, SignatureHelp, SignatureInformation,
    SymbolInformation, SEMANTIC_TOKEN_TYPES,
};
use dbt_ls_parser::{Keyword, Token};

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
        .dependents_of(&model)
        .filter(|node| !node.uri.as_os_str().is_empty())
        .map(|node| Location {
            uri: file_uri(&node.uri),
            range: Range::default(),
        })
        .collect()
}

/// Dangling-ref warnings (P1). Suppressed while the first context refresh is
/// still pending (an empty model map would flag everything).
pub fn dangling_ref_diagnostics(ctx: &DbtContext, doc: &Document) -> Vec<Diagnostic> {
    if ctx.model_detail_map.is_empty() {
        return Vec::new();
    }
    doc.parsed
        .tokens
        .iter()
        .filter(|t| t.kind == TokenKind::Ref)
        .filter(|t| !ctx.model_detail_map.contains_key(doc.parsed.literal(t)))
        .map(|t| Diagnostic {
            range: token_range(t),
            message: format!("model '{}' not found in the project", doc.parsed.literal(t)),
            severity: diagnostic_severity::WARNING,
            code: "dangling-ref".to_string(),
            source: "dbt-language-server".to_string(),
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
    for raw in dbt_ls_parser::jinja_scan::extract_macros(&doc.text) {
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

/// textDocument/foldingRange: multi-line paren pairs (CTE bodies) — token-
/// based v1; the tree-sitter phase upgrades this (06 §3.6).
pub fn folding_ranges(doc: &Document) -> Vec<FoldingRange> {
    let mut stack: Vec<u32> = Vec::new();
    let mut out = Vec::new();
    for t in &doc.parsed.tokens {
        match t.kind {
            TokenKind::Symbol('(') => stack.push(t.line),
            TokenKind::Symbol(')') => {
                if let Some(open_line) = stack.pop() {
                    if t.line > open_line {
                        out.push(FoldingRange {
                            start_line: open_line,
                            end_line: t.line,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    out.sort_by_key(|r| (r.start_line, r.end_line));
    out.dedup_by_key(|r| (r.start_line, r.end_line));
    out
}

/// textDocument/selectionRange: token → line → document chain per position.
pub fn selection_ranges(doc: &Document, positions: &[Position]) -> Vec<SelectionRange> {
    let line_count = doc.text.split('\n').count() as u32;
    positions
        .iter()
        .map(|pos| {
            let document = SelectionRange {
                range: Range {
                    start: Position::default(),
                    end: Position {
                        line: line_count.saturating_sub(1),
                        character: 0,
                    },
                },
                parent: None,
            };
            let line_len = doc
                .text
                .split('\n')
                .nth(pos.line as usize)
                .map(|l| l.len() as u32)
                .unwrap_or(0);
            let line = SelectionRange {
                range: Range {
                    start: Position {
                        line: pos.line,
                        character: 0,
                    },
                    end: Position {
                        line: pos.line,
                        character: line_len,
                    },
                },
                parent: Some(Box::new(document)),
            };
            match doc.parsed.find_token_at_cursor(pos.line, pos.character) {
                Some(idx) => SelectionRange {
                    range: token_range(&doc.parsed.tokens[idx]),
                    parent: Some(Box::new(line)),
                },
                None => line,
            }
        })
        .collect()
}

/// textDocument/semanticTokens/full: LSP delta encoding over the refined
/// token stream (token-based v1, legend in dbt_ls_lsp::SEMANTIC_TOKEN_TYPES).
pub fn semantic_tokens(doc: &Document) -> SemanticTokens {
    let type_index = |name: &str| {
        SEMANTIC_TOKEN_TYPES
            .iter()
            .position(|t| *t == name)
            .unwrap() as u32
    };
    let keyword = type_index("keyword");
    let macro_t = type_index("macro");
    let variable = type_index("variable");
    let string_t = type_index("string");
    let number = type_index("number");
    let namespace = type_index("namespace");

    let mut data = Vec::new();
    let (mut prev_line, mut prev_col) = (0u32, 0u32);
    for t in &doc.parsed.tokens {
        let token_type = match t.kind {
            TokenKind::Word(kw) if kw != Keyword::NoKeyword => keyword,
            TokenKind::Config => keyword,
            TokenKind::Macro => macro_t,
            TokenKind::Package => namespace,
            TokenKind::Ref | TokenKind::SourceName | TokenKind::SourceTable | TokenKind::Var => {
                variable
            }
            TokenKind::StringLit => string_t,
            TokenKind::Number => number,
            _ => continue,
        };
        if doc.parsed.literal(t).contains('\n') {
            continue; // single-line encoding only
        }
        let delta_line = t.line - prev_line;
        let delta_col = if delta_line == 0 {
            t.column - prev_col
        } else {
            t.column
        };
        data.extend_from_slice(&[delta_line, delta_col, t.len(), token_type, 0]);
        (prev_line, prev_col) = (t.line, t.column);
    }
    SemanticTokens { data }
}

/// textDocument/signatureHelp (P2): nearest unclosed macro call before the
/// cursor; parameters split from the stored signature text.
pub fn signature_help(ctx: &DbtContext, doc: &Document, pos: Position) -> Option<SignatureHelp> {
    // walk tokens positioned before the cursor, backwards
    let before = doc
        .parsed
        .tokens
        .iter()
        .enumerate()
        .take_while(|(_, t)| t.line < pos.line || (t.line == pos.line && t.column < pos.character))
        .collect::<Vec<_>>();
    let mut depth = 0i32;
    let mut commas = 0u32;
    for (idx, t) in before.iter().rev() {
        match t.kind {
            TokenKind::Symbol(')') => depth += 1,
            TokenKind::Symbol(',') if depth == 0 => commas += 1,
            TokenKind::Symbol('(') => {
                if depth == 0 {
                    // the call ident sits right before this paren
                    let callee = idx.checked_sub(1).map(|i| &doc.parsed.tokens[i])?;
                    if callee.kind != TokenKind::Macro {
                        return None;
                    }
                    let name = doc.parsed.literal(callee);
                    let package = doc
                        .parsed
                        .lookback_find(*idx - 1, TokenKind::Package, 2)
                        .map(|t| doc.parsed.literal(t).to_string())
                        .unwrap_or_else(|| ctx.project.name.clone());
                    let details = ctx.macro_detail_map.get(&package)?.get(name)?;
                    return Some(signature_from_text(&details.description, commas));
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    None
}

fn signature_from_text(signature: &str, active_parameter: u32) -> SignatureHelp {
    let inner = signature
        .split_once('(')
        .map(|(_, rest)| rest.trim_end_matches(')'))
        .unwrap_or("");
    let parameters = split_top_level(inner)
        .into_iter()
        .map(|p| ParameterInformation {
            label: p.trim().to_string(),
        })
        .filter(|p| !p.label.is_empty())
        .collect();
    SignatureHelp {
        signatures: vec![SignatureInformation {
            label: signature.to_string(),
            parameters,
        }],
        active_signature: 0,
        active_parameter,
    }
}

fn split_top_level(args: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();
    for c in args.chars() {
        match c {
            '(' | '[' | '{' => {
                depth += 1;
                current.push(c);
            }
            ')' | ']' | '}' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => {
                out.push(std::mem::take(&mut current));
            }
            _ => current.push(c),
        }
    }
    if !current.trim().is_empty() {
        out.push(current);
    }
    out
}

/// completionItem/resolve (P2): fill documentation lazily from the maps.
pub fn resolve_completion_documentation(
    ctx: &DbtContext,
    data: &serde_json::Value,
) -> Option<String> {
    let kind = data.get("kind")?.as_str()?;
    let name = data.get("name")?.as_str()?;
    match kind {
        "model" => ctx
            .model_detail_map
            .get(name)
            .map(|m| m.description.clone()),
        "source" => {
            let table = data.get("table")?.as_str()?;
            let source = ctx.source_detail_map.get(name)?;
            let t = source.tables.get(table)?;
            Some(format!(
                "{}\n\nTable: {}\n{}",
                source.description, t.name, t.description
            ))
        }
        "variable" => ctx
            .variable_detail_map
            .get(name)
            .map(|v| display_value(&v.value)),
        "macro" => {
            let package = data
                .get("package")
                .and_then(|p| p.as_str())
                .unwrap_or(&ctx.project.name);
            ctx.macro_detail_map
                .get(package)?
                .get(name)
                .map(|m| m.description.clone())
        }
        "function" => ctx.dialect.function_docs().get(name).cloned(),
        _ => None,
    }
}

fn token_range(t: &Token) -> Range {
    Range {
        start: Position {
            line: t.line,
            character: t.column,
        },
        end: Position {
            line: t.line,
            character: t.column + t.len(),
        },
    }
}
