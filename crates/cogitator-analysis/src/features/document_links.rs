//! textDocument/documentLink (01 §6.3): every ref()/source(), macro call
//! and doc() becomes a clickable link to its definition — no resolve
//! round-trip. [`document_links_at`] adds the plain-scalar ref()/source()
//! of properties YAML (relationship tests, exposures), which are no
//! template tokens.

use cogitator_lsp::DocumentLink;
use cogitator_parser::TokenKind;

use super::references::files::{is_yaml, path_of, project_of_path, resolve_macro};
use super::references::occur::qualifier;
use super::references::yaml_calls::yaml_calls;
use super::{file_uri, token_range, DbtContext, Document};

pub fn document_links(ctx: &DbtContext, doc: &Document) -> Vec<DocumentLink> {
    document_links_in(ctx, doc, "")
}

/// [`document_links`] knowing the document's uri: package files resolve
/// their bare macro calls in their own package, and YAML adds its plain
/// scalars.
pub fn document_links_at(ctx: &DbtContext, doc: &Document, uri: &str) -> Vec<DocumentLink> {
    let mut out = document_links_in(ctx, doc, uri);
    if path_of(uri).is_some_and(|p| is_yaml(&p)) {
        out.extend(yaml_links(ctx, &doc.text));
    }
    out.sort_by_key(|l| (l.range.start.line, l.range.start.character));
    out
}

fn document_links_in(ctx: &DbtContext, doc: &Document, uri: &str) -> Vec<DocumentLink> {
    let project =
        path_of(uri).map_or_else(|| ctx.project.name.clone(), |p| project_of_path(ctx, &p));
    let mut out = Vec::new();
    let mut source: Option<String> = None;
    for (idx, token) in doc.parsed.tokens.iter().enumerate() {
        let literal = doc.parsed.literal(token);
        match token.kind {
            TokenKind::Ref => {
                let Some(details) = ctx.model_detail_map.get(literal) else {
                    continue;
                };
                if details.uri.as_os_str().is_empty() {
                    continue;
                }
                out.push(DocumentLink {
                    range: token_range(token),
                    target: file_uri(&details.uri),
                    tooltip: Some(format!("model {literal}")),
                });
            }
            TokenKind::SourceName => {
                source = Some(literal.to_string());
                if let Some(details) = ctx.source_detail_map.get(literal) {
                    out.push(DocumentLink {
                        range: token_range(token),
                        target: file_uri(&details.uri),
                        tooltip: Some(format!("source {literal}")),
                    });
                }
            }
            TokenKind::SourceTable => {
                let Some(table) = source
                    .as_deref()
                    .and_then(|s| ctx.source_detail_map.get(s))
                    .and_then(|s| s.tables.get(literal))
                else {
                    continue;
                };
                out.push(DocumentLink {
                    range: token_range(token),
                    target: file_uri(&table.uri),
                    tooltip: Some(format!("source table {literal}")),
                });
            }
            TokenKind::Macro => {
                let qualifier = qualifier(&doc.parsed, idx);
                let Some(details) = resolve_macro(ctx, &project, qualifier, literal) else {
                    continue;
                };
                out.push(DocumentLink {
                    range: token_range(token),
                    target: file_uri(&details.uri),
                    tooltip: Some(format!("macro {}.{literal}", details.project_name)),
                });
            }
            TokenKind::Doc => {
                let Some(details) = ctx.doc_detail_map.get(literal) else {
                    continue;
                };
                out.push(DocumentLink {
                    range: token_range(token),
                    target: file_uri(&details.uri),
                    tooltip: Some(format!("docs block {literal}")),
                });
            }
            TokenKind::Function => {
                let Some((path, _)) = ctx
                    .function_detail_map
                    .get(literal)
                    .and_then(|f| f.definition())
                else {
                    continue;
                };
                out.push(DocumentLink {
                    range: token_range(token),
                    target: file_uri(&path),
                    tooltip: Some(format!("function {literal}")),
                });
            }
            _ => {}
        }
    }
    out
}

fn yaml_links(ctx: &DbtContext, text: &str) -> Vec<DocumentLink> {
    yaml_calls(text, &ctx.dialect)
        .into_iter()
        .filter_map(|call| {
            let (target, tooltip) = match call.kind {
                TokenKind::Ref => {
                    let m = ctx.model_detail_map.get(&call.literal)?;
                    (m.uri.clone(), format!("model {}", call.literal))
                }
                TokenKind::SourceName => {
                    let s = ctx.source_detail_map.get(&call.literal)?;
                    (s.uri.clone(), format!("source {}", call.literal))
                }
                TokenKind::SourceTable => {
                    let t = ctx
                        .source_detail_map
                        .get(call.source.as_deref()?)?
                        .tables
                        .get(&call.literal)?;
                    (t.uri.clone(), format!("source table {}", call.literal))
                }
                _ => return None,
            };
            (!target.as_os_str().is_empty()).then(|| DocumentLink {
                range: call.range,
                target: file_uri(&target),
                tooltip: Some(tooltip),
            })
        })
        .collect()
}
