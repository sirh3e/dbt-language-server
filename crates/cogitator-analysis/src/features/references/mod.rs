//! textDocument/references beyond `ref()` (FEATURE_REQUESTS P1): sources
//! (whole or one table), macros, docs blocks and documented columns.
//! [`references_at`] answers those kinds itself and hands everything else
//! to the model references of `navigation.rs` unchanged. Files of other
//! documents are read from disk, the open document from its buffer.

mod columns;
pub(crate) mod files;
pub(crate) mod macros;
pub(crate) mod occur;
pub(crate) mod target;
pub(crate) mod yaml_calls;

use std::path::{Path, PathBuf};

use cogitator_lsp::{Location, Position, Range};
use cogitator_parser::{DocumentParser, JinjaSqlParser, TokenKind};

use files::{is_yaml, node_files, yaml_files, Reader};
use occur::{doc_uses, function_uses, name_span, source_uses};
use target::{target_at, Target};

use super::{file_uri, model_name_from_uri, DbtContext, Document};

/// References of whatever the cursor names; `include_declaration` adds
/// the definition (the YAML entry, the macro header, the docs block, the
/// model file).
pub fn references_at(
    ctx: &DbtContext,
    doc: Option<&Document>,
    uri: &str,
    pos: Position,
    include_declaration: bool,
) -> Vec<Location> {
    let reader = Reader::disk(doc.map(|d| (uri, &*d.text)));
    let target = doc.and_then(|d| target_at(ctx, d, uri, pos));
    let Some(target) = target else {
        return model_references(ctx, doc, uri, pos, include_declaration);
    };
    let mut out = match target {
        Target::Source { source, table } => {
            source_references(ctx, &reader, &source, table.as_deref(), include_declaration)
        }
        Target::Macro { package, name } => {
            macro_references(ctx, &reader, &package, &name, include_declaration)
        }
        Target::Doc(name) => doc_references(ctx, &reader, doc, uri, &name, include_declaration),
        Target::Function(name) => {
            function_references(ctx, &reader, doc, uri, &name, include_declaration)
        }
        Target::Column { model, column } => {
            columns::column_references(ctx, &reader, &model, &column, include_declaration)
        }
    };
    sort_locations(&mut out);
    out
}

/// The `ref()` references, plus the model file as its declaration.
fn model_references(
    ctx: &DbtContext,
    doc: Option<&Document>,
    uri: &str,
    pos: Position,
    include_declaration: bool,
) -> Vec<Location> {
    let mut out = super::references(ctx, doc, uri, pos);
    if include_declaration {
        let model = doc
            .and_then(|d| {
                let idx = d.parsed.find_token_at_cursor(pos.line, pos.character)?;
                let t = &d.parsed.tokens[idx];
                (t.kind == TokenKind::Ref).then(|| d.parsed.literal(t).to_string())
            })
            .or_else(|| model_name_from_uri(uri));
        let declared = model
            .and_then(|m| ctx.model_detail_map.get(&m))
            .filter(|d| !d.uri.as_os_str().is_empty());
        if let Some(details) = declared {
            out.insert(
                0,
                Location {
                    uri: file_uri(&details.uri),
                    range: Range::default(),
                },
            );
        }
    }
    out
}

fn sort_locations(out: &mut Vec<Location>) {
    out.sort_by(|a, b| {
        (&a.uri, a.range.start.line, a.range.start.character).cmp(&(
            &b.uri,
            b.range.start.line,
            b.range.start.character,
        ))
    });
    out.dedup();
}

fn locations(path: &Path, ranges: Vec<Range>) -> impl Iterator<Item = Location> + '_ {
    ranges.into_iter().map(move |range| Location {
        uri: file_uri(path),
        range,
    })
}

/// The declaration's name range, verified against its file's text.
fn declared(reader: &Reader<'_>, path: &Path, at: Position, name: &str) -> Location {
    let range = reader
        .read(path)
        .and_then(|text| name_span(&text, at, name))
        .unwrap_or_else(|| Range::zero_width(at));
    Location {
        uri: file_uri(path),
        range,
    }
}

fn source_references(
    ctx: &DbtContext,
    reader: &Reader<'_>,
    source: &str,
    table: Option<&str>,
    include_declaration: bool,
) -> Vec<Location> {
    let (_, incoming) = ctx.graph.node_edges(crate::graph::NodeKind::Source, source);
    let mut files: Vec<PathBuf> = node_files(incoming.iter().map(|(_, n)| *n));
    files.extend(yaml_files(ctx));
    files.sort();
    files.dedup();
    let mut out: Vec<Location> = Vec::new();
    for path in &files {
        let Some(text) = reader.read(path) else {
            continue;
        };
        let ranges = if is_yaml(path) {
            let mut ranges: Vec<Range> = yaml_calls::yaml_calls(&text, &ctx.dialect)
                .into_iter()
                .filter(|c| match table {
                    None => c.kind == TokenKind::SourceName && c.literal == source,
                    Some(t) => {
                        c.kind == TokenKind::SourceTable
                            && c.literal == t
                            && c.source.as_deref() == Some(source)
                    }
                })
                .map(|c| c.range)
                .collect();
            ranges.extend(parsed_ranges(ctx, &text, |p| source_uses(p, source, table)));
            ranges
        } else {
            parsed_ranges(ctx, &text, |p| source_uses(p, source, table))
        };
        out.extend(locations(path, ranges));
    }
    if include_declaration {
        if let Some(details) = ctx.source_detail_map.get(source) {
            match table.and_then(|t| details.tables.get(t)) {
                Some(t) => out.push(declared(reader, &t.uri, t.range.start, &t.name)),
                None if table.is_none() => out.push(declared(
                    reader,
                    &details.uri,
                    details.range.start,
                    &details.name,
                )),
                None => {}
            }
        }
    }
    out
}

fn parsed_ranges(
    ctx: &DbtContext,
    text: &str,
    find: impl Fn(&cogitator_parser::ParsedDocument) -> Vec<Range>,
) -> Vec<Range> {
    JinjaSqlParser
        .parse(text, &ctx.dialect)
        .map(|p| find(&p))
        .unwrap_or_default()
}

fn macro_references(
    ctx: &DbtContext,
    reader: &Reader<'_>,
    package: &str,
    name: &str,
    include_declaration: bool,
) -> Vec<Location> {
    let mut out: Vec<Location> = macros::call_sites(ctx, reader, package, name)
        .into_iter()
        .flat_map(|site| {
            let path = site.path;
            site.ranges.into_iter().map(move |range| Location {
                uri: file_uri(&path),
                range,
            })
        })
        .collect();
    if include_declaration {
        if let Some((path, range)) = macros::definition_range(ctx, package, name) {
            out.push(Location {
                uri: file_uri(&path),
                range,
            });
        }
        if package == ctx.project.name {
            for path in yaml_files(ctx) {
                if let Some(text) = reader.read(&path) {
                    out.extend(locations(&path, yaml_calls::macro_entries(&text, name)));
                }
            }
        }
    }
    out
}

/// `function('name')` calls in every file of the graph (models, tests,
/// analyses) and the open document; the declaration is the body file and
/// the `functions:` entry.
fn function_references(
    ctx: &DbtContext,
    reader: &Reader<'_>,
    doc: Option<&Document>,
    uri: &str,
    name: &str,
    include_declaration: bool,
) -> Vec<Location> {
    let mut files = node_files(ctx.graph.nodes());
    if let Some(path) = files::path_of(uri).filter(|_| doc.is_some()) {
        files.push(path);
    }
    files.sort();
    files.dedup();
    let mut out: Vec<Location> = files
        .iter()
        .filter(|path| !is_yaml(path))
        .filter_map(|path| reader.read(path).map(|text| (path, text)))
        .flat_map(|(path, text)| {
            locations(path, parsed_ranges(ctx, &text, |p| function_uses(p, name)))
                .collect::<Vec<_>>()
        })
        .collect();
    if include_declaration {
        if let Some(details) = ctx.function_detail_map.get(name) {
            if let Some(body) = &details.body {
                out.push(Location {
                    uri: file_uri(body),
                    range: Range::default(),
                });
            }
            if let (Some(yaml), Some(entry)) = (&details.yaml, &details.entry) {
                out.push(declared(reader, yaml, entry.name_pos, name));
            }
        }
    }
    out
}

fn doc_references(
    ctx: &DbtContext,
    reader: &Reader<'_>,
    doc: Option<&Document>,
    uri: &str,
    name: &str,
    include_declaration: bool,
) -> Vec<Location> {
    let mut files = yaml_files(ctx);
    if let Some(path) = files::path_of(uri).filter(|_| doc.is_some()) {
        files.push(path);
    }
    files.sort();
    files.dedup();
    let mut out: Vec<Location> = Vec::new();
    for path in &files {
        if let Some(text) = reader.read(path) {
            out.extend(locations(
                path,
                parsed_ranges(ctx, &text, |p| doc_uses(p, name)),
            ));
        }
    }
    if include_declaration {
        if let Some(details) = ctx.doc_detail_map.get(name) {
            out.push(declared(reader, &details.uri, details.range.start, name));
        }
    }
    out
}
