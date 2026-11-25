//! "Extract CTE 'x' to a model" (`refactor.extract`), on a CTE's name
//! (its definition or a use): the CTE's body becomes `x.sql` beside the
//! model, the CTE leaves this file, and every FROM/JOIN of `x` reads
//! `{{ ref('x') }}` instead, aliased `x` where it had no alias, so
//! columns qualified `x.col` still resolve.
//!
//! Refused where the result would not stand on its own: a body that
//! reads another CTE of this file (it would dangle in the new model), a
//! name a model already has, a CTE with a column list, or a name that is
//! not a plain identifier.

use std::path::Path;

use cogitator_lsp::{
    CodeAction, CreateFile, DocumentChange, Position, Range, TextDocumentEdit, TextEdit,
    WorkspaceEdit,
};
use cogitator_parser::{Keyword, Token, TokenKind};

use super::super::{file_uri, DbtContext, Document};
use super::yaml_text::plain_name;

pub fn extract_cte_action(
    ctx: &DbtContext,
    doc: &Document,
    uri: &str,
    range: Range,
) -> Option<CodeAction> {
    let parsed = &doc.parsed;
    let at = parsed.find_token_at_cursor(range.start.line, range.start.character)?;
    let name = plain_name(parsed.literal(&parsed.tokens[at]))?;
    if ctx.model_detail_map.contains_key(name) {
        return None;
    }
    let extraction = extraction(doc, name, name)?;
    let path = cogitator_lsp::uri_to_fs_path(uri)?;
    let new_uri = file_uri(&Path::new(&path).parent()?.join(format!("{name}.sql")));
    Some(CodeAction {
        title: format!("Extract CTE '{name}' to a model"),
        kind: Some("refactor.extract"),
        diagnostics: None,
        edit: Some(WorkspaceEdit {
            document_changes: vec![
                DocumentChange::Create(CreateFile::new(new_uri.clone())),
                DocumentChange::Edit(TextDocumentEdit::new(
                    new_uri,
                    vec![TextEdit {
                        range: Range::zero_width(Position {
                            line: 0,
                            character: 0,
                        }),
                        new_text: format!("{}\n", extraction.body),
                    }],
                )),
                DocumentChange::Edit(TextDocumentEdit::new(uri.to_string(), extraction.edits)),
            ],
        }),
        command: None,
        is_preferred: None,
        data: None,
    })
}

/// A CTE taken out of its model: its body's SQL, and the edits of the
/// model that remove it and read `model` in its place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extraction {
    pub body: String,
    pub edits: Vec<TextEdit>,
}

/// The CTE `cte` of `doc` as the model `model`: refused where the body
/// would not stand alone (it reads another CTE, it has a column list).
/// Each FROM/JOIN of the CTE becomes `{{ ref('<model>') }}`, aliased by
/// the CTE's name where it had no alias.
pub fn extraction(doc: &Document, cte: &str, model: &str) -> Option<Extraction> {
    let parsed = &doc.parsed;
    let tokens = &parsed.tokens;
    let name = cte;
    let def = parsed.def_tokens.get(name)?;
    let def_i = tokens.iter().position(|t| t.start == def.start)?;
    // `name as (`: anything else (a column list) is left alone
    let as_i = def_i + 1;
    let open_i = def_i + 2;
    if tokens.get(as_i)?.kind != TokenKind::Word(Keyword::AS)
        || tokens.get(open_i)?.kind != TokenKind::Symbol('(')
    {
        return None;
    }
    let close_i = matching_paren(tokens, open_i)?;
    let body_tokens = &tokens[open_i + 1..close_i];
    let reads_another = body_tokens.iter().any(|t| {
        matches!(t.kind, TokenKind::Word(_) | TokenKind::Ident)
            && parsed.literal(t) != name
            && parsed.def_tokens.contains_key(parsed.literal(t))
    });
    if reads_another || body_tokens.is_empty() {
        return None;
    }
    let text = &parsed.text;
    let body = text[tokens[open_i].end as usize..tokens[close_i].start as usize]
        .trim()
        .to_string();

    let mut edits = vec![TextEdit {
        range: cte_removal(tokens, def_i, close_i)?,
        new_text: String::new(),
    }];
    for (i, t) in tokens.iter().enumerate() {
        let is_use = i != def_i
            && matches!(t.kind, TokenKind::Word(_) | TokenKind::Ident)
            && parsed.literal(t) == name
            && i > 0
            && matches!(
                tokens[i - 1].kind,
                TokenKind::Word(Keyword::FROM) | TokenKind::Word(Keyword::JOIN)
            );
        if !is_use {
            continue;
        }
        let aliased = matches!(
            tokens.get(i + 1).map(|n| n.kind),
            Some(TokenKind::Word(Keyword::AS)) | Some(TokenKind::Word(Keyword::NoKeyword))
        );
        let call = format!("{{{{ ref('{model}') }}}}");
        edits.push(TextEdit {
            range: span(t, t),
            new_text: if aliased {
                call
            } else {
                format!("{call} as {name}")
            },
        });
    }
    Some(Extraction { body, edits })
}

/// What leaves the file with the CTE `tokens[def_i..=close_i]`: with
/// the comma after it when another CTE follows; with the comma before
/// it when it is the last of several; with its `with` when it is the
/// only one. Each range ends where the next token starts, so the
/// layout around it stays as it was.
fn cte_removal(tokens: &[Token], def_i: usize, close_i: usize) -> Option<Range> {
    let next = tokens.get(close_i + 1)?;
    if next.kind == TokenKind::Symbol(',') {
        let after = tokens.get(close_i + 2)?;
        return Some(between(&tokens[def_i], after));
    }
    let before = tokens.get(def_i.checked_sub(1)?)?;
    match before.kind {
        TokenKind::Symbol(',') => Some(span(before, &tokens[close_i])),
        TokenKind::Word(Keyword::WITH) => Some(between(before, next)),
        _ => None,
    }
}

/// The token at the matching close of the paren at `open`.
fn matching_paren(tokens: &[Token], open: usize) -> Option<usize> {
    let mut depth = 0i32;
    for (i, t) in tokens.iter().enumerate().skip(open) {
        match t.kind {
            TokenKind::Symbol('(') => depth += 1,
            TokenKind::Symbol(')') => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

fn pos(t: &Token) -> Position {
    Position {
        line: t.line,
        character: t.column,
    }
}

/// From the start of `first` to the end of `last`.
fn span(first: &Token, last: &Token) -> Range {
    Range {
        start: pos(first),
        end: Position {
            line: last.line,
            character: last.column + last.len(),
        },
    }
}

/// From the start of `first` to the start of `next`.
fn between(first: &Token, next: &Token) -> Range {
    Range {
        start: pos(first),
        end: pos(next),
    }
}
