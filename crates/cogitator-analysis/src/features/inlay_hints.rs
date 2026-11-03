//! textDocument/inlayHint (01 §6.3): the resolved warehouse relation
//! after each ref()/source() call, and parameter names at macro call
//! sites. Pure — the shell resolves the active target first.

use cogitator_lsp::{inlay_hint_kind, InlayHint, Position, Range};
use cogitator_parser::TokenKind;

use super::relation::TargetRelation;
use super::{relation_info, DbtContext, Document};

pub fn inlay_hints(
    ctx: &DbtContext,
    doc: &Document,
    uri: &str,
    range: Range,
    target: Option<&TargetRelation>,
) -> Vec<InlayHint> {
    let whole_doc = range.is_zero();
    let within = |line: u32| whole_doc || (range.start.line <= line && line <= range.end.line);
    let mut out = Vec::new();
    for (i, token) in doc.parsed.tokens.iter().enumerate() {
        if !within(token.line) {
            continue;
        }
        match token.kind {
            // the relation the call resolves to, after the closing quote
            TokenKind::Ref | TokenKind::SourceTable => {
                let pos = Position {
                    line: token.line,
                    character: token.column,
                };
                let Some(info) = relation_info(ctx, Some(doc), uri, Some(pos), target) else {
                    continue;
                };
                out.push(InlayHint {
                    position: Position {
                        line: token.line,
                        character: token.column + token.len() + 1,
                    },
                    label: format!("→ {}", info.relation),
                    kind: Some(inlay_hint_kind::TYPE),
                    padding_left: true,
                    padding_right: false,
                });
            }
            // parameter names for positional macro arguments
            TokenKind::Macro => out.extend(macro_param_hints(ctx, doc, i, token)),
            _ => {}
        }
    }
    out
}

fn macro_param_hints(
    ctx: &DbtContext,
    doc: &Document,
    i: usize,
    token: &cogitator_parser::Token,
) -> Vec<InlayHint> {
    let name = doc.parsed.literal(token);
    let Some(params) = ctx
        .macro_detail_map
        .values()
        .find_map(|package| package.get(name))
        .map(|details| &details.params)
    else {
        return Vec::new();
    };
    let tokens = &doc.parsed.tokens;
    // arguments start after `(`; one hint per positional arg start
    let mut j = i + 1;
    if !matches!(tokens.get(j).map(|t| t.kind), Some(TokenKind::Symbol('('))) {
        return Vec::new();
    }
    j += 1;
    let mut depth = 0usize;
    let mut arg = 0usize;
    let mut at_arg_start = true;
    let mut out = Vec::new();
    while let Some(t) = tokens.get(j) {
        match t.kind {
            TokenKind::Symbol('(') => depth += 1,
            TokenKind::Symbol(')') if depth == 0 => break,
            TokenKind::Symbol(')') => depth -= 1,
            TokenKind::Symbol(',') if depth == 0 => {
                arg += 1;
                at_arg_start = true;
            }
            TokenKind::Symbol('=') => {
                // named argument: drop the hint we just placed for it
                if let Some(last) = out.last() {
                    let _: &InlayHint = last;
                    out.pop();
                }
            }
            TokenKind::JinjaClose(_) => break,
            _ if at_arg_start => {
                at_arg_start = false;
                if let Some(param) = params.get(arg) {
                    out.push(InlayHint {
                        position: Position {
                            line: t.line,
                            character: t.column.saturating_sub(if t.kind == TokenKind::StringLit {
                                1
                            } else {
                                0
                            }),
                        },
                        label: format!("{}:", param.name),
                        kind: Some(inlay_hint_kind::PARAMETER),
                        padding_left: false,
                        padding_right: true,
                    });
                }
            }
            _ => {}
        }
        j += 1;
    }
    out
}
