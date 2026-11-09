//! The lineage masking front half: named placeholders for every
//! ref()/source() region so sqlparser sees plain SQL, with the
//! placeholder → relation map for the resolver.

use std::collections::HashMap;

use cogitator_parser::jinja_spans::{blanked, NOT_SQL};
use cogitator_parser::{JinjaKind, ParsedDocument, Token, TokenKind};

use super::Relation;

/// Rebuild the SQL with named placeholder identifiers for Jinja expressions.
pub(super) fn mask_for_lineage(
    doc: &ParsedDocument,
) -> Option<(String, HashMap<String, Relation>)> {
    let text: &str = &doc.text;
    let mut masked = String::with_capacity(text.len());
    let mut placeholders = HashMap::new();
    let mut counter = 0usize;

    let mut pos = 0usize;
    let mut i = 0;
    while i < doc.tokens.len() {
        let token = &doc.tokens[i];
        if let TokenKind::JinjaOpen(kind) = token.kind {
            let close = doc.tokens[i..]
                .iter()
                .position(|t| matches!(t.kind, TokenKind::JinjaClose(k) if k == kind))
                .map(|off| i + off)?;
            let (start, end) = (token.start as usize, doc.tokens[close].end as usize);
            masked.push_str(&text[pos..start.max(pos).min(text.len())]);
            match kind {
                // config() renders to nothing, not to a relation
                JinjaKind::Expr if is_config_call(doc, i) => {}
                JinjaKind::Expr => {
                    let region = &doc.tokens[i..=close];
                    let relation = relation_in_region(doc, region);
                    let placeholder = format!("__dbt_rel_{counter}__");
                    counter += 1;
                    if let Some(relation) = relation {
                        placeholders.insert(placeholder.clone(), relation);
                    }
                    masked.push_str(&placeholder);
                }
                JinjaKind::Stmt => {
                    // config/set blocks blank; control flow declines (07 §3.2)
                    let tag = doc.tokens[i + 1..close]
                        .iter()
                        .find(|t| t.kind == TokenKind::Ident || t.kind == TokenKind::Config)
                        .map(|t| doc.literal(t))
                        .unwrap_or("");
                    if !matches!(tag, "config" | "set" | "endset") {
                        return None;
                    }
                }
            }
            pos = end;
            i = close + 1;
            continue;
        }
        i += 1;
    }
    masked.push_str(&text[pos.min(text.len())..]);

    // comments and raw tags are not tokens; blank them in the rebuilt text
    let masked = blanked(&masked, NOT_SQL);
    Some((masked, placeholders))
}

pub(super) fn relation_in_region(doc: &ParsedDocument, region: &[Token]) -> Option<Relation> {
    let reference = region.iter().find(|t| t.kind == TokenKind::Ref);
    if let Some(reference) = reference {
        return Some(Relation::Model(doc.literal(reference).to_string()));
    }
    let source = region.iter().find(|t| t.kind == TokenKind::SourceName)?;
    let table = region.iter().find(|t| t.kind == TokenKind::SourceTable)?;
    Some(Relation::Source(
        doc.literal(source).to_string(),
        doc.literal(table).to_string(),
    ))
}

fn is_config_call(doc: &ParsedDocument, open: usize) -> bool {
    doc.tokens
        .get(open + 1)
        .is_some_and(|t| t.kind == TokenKind::Config)
}

// ---------------------------------------------------------------------------
// Resolution
// ---------------------------------------------------------------------------
