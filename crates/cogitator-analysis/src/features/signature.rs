//! textDocument/signatureHelp for macro calls (FEATURE_REQUESTS P2).

use cogitator_lsp::{ParameterInformation, Position, SignatureHelp, SignatureInformation};
use cogitator_parser::TokenKind;

use super::{DbtContext, Document};

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
