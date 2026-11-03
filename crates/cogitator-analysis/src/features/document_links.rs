//! textDocument/documentLink (01 §6.3): every ref()/source() becomes a
//! clickable link to its definition — no resolve round-trip.

use cogitator_lsp::DocumentLink;
use cogitator_parser::TokenKind;

use super::{file_uri, token_range, DbtContext, Document};

pub fn document_links(ctx: &DbtContext, doc: &Document) -> Vec<DocumentLink> {
    let mut out = Vec::new();
    let mut source: Option<String> = None;
    for token in &doc.parsed.tokens {
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
            _ => {}
        }
    }
    out
}
