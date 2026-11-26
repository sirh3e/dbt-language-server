//! "Inline model 'x' as a CTE" (`refactor.inline`), on a `{{ ref('x') }}`:
//! the reverse of "Extract CTE". x's SQL becomes the CTE `x` of this
//! model (the first of its `with`, or a new `with`), the call reads
//! `x`, x's SQL file is deleted (a `DeleteFile` resource operation) and
//! its properties entry removed.
//!
//! Only a small, plain model that nothing else needs is inlined;
//! refused where:
//! - x is not a model of the project (a seed, a snapshot, a package's);
//! - anything but this model depends on x (models, singular tests,
//!   analyses, exposures: the graph's downstream);
//! - x is not small: more than [`INLINE_MAX_LINES`] lines, or any Jinja
//!   statement (`{% … %}`: control flow, `set`, a config block) or a
//!   `config(` call;
//! - x has tests (a `tests:`/`data_tests:` key anywhere in its entry),
//!   versions, an enforced contract;
//! - this file defines a CTE named `x` already, or starts `with recursive`.
//!
//! x's own `with` needs no merging: it stays inside the new CTE's
//! parentheses, which every warehouse dbt supports reads.

use std::path::Path;

use cogitator_lsp::{
    CodeAction, DeleteFile, DocumentChange, Position, Range, TextDocumentEdit, TextEdit,
    WorkspaceEdit,
};
use cogitator_parser::{JinjaKind, Keyword, Token, TokenKind};

use super::super::{file_uri, model_name_from_uri, DbtContext, Document};
use super::yaml_text::{entry_at, lines_range, plain_name};
use crate::context::ModelKind;

/// The most lines a model may have to be inlined.
pub const INLINE_MAX_LINES: usize = 60;

pub fn inline_model_action(
    ctx: &DbtContext,
    doc: &Document,
    uri: &str,
    range: Range,
    read: &dyn Fn(&Path) -> Option<String>,
) -> Option<CodeAction> {
    let here = model_name_from_uri(uri)?;
    let parsed = &doc.parsed;
    let idx = parsed.find_token_at_cursor(range.start.line, range.start.character)?;
    let token = &parsed.tokens[idx];
    if token.kind != TokenKind::Ref {
        return None;
    }
    let name = plain_name(parsed.literal(token))?.to_string();
    let details = ctx.model_detail_map.get(&name)?;
    if name == here
        || details.kind != ModelKind::Model
        || details.uri.as_os_str().is_empty()
        || details.contract_enforced
        || parsed.def_tokens.contains_key(name.as_str())
    {
        return None;
    }
    let root = ctx.project_root.as_deref();
    if root.is_some_and(|r| !details.uri.starts_with(r))
        || details
            .uri
            .components()
            .any(|c| c.as_os_str() == "dbt_packages")
    {
        return None; // a package's model is not this project's to delete
    }
    if ctx.graph.downstream_of(&name).any(|n| n.name != here) {
        return None;
    }
    let sql = read(&details.uri)?;
    if sql.lines().count() > INLINE_MAX_LINES || sql.contains("{%") || sql.contains("config(") {
        return None;
    }
    let mut yaml_change = None;
    if let Some(schema) = &details.schema_uri {
        let text = read(schema)?;
        let lines: Vec<&str> = text.lines().collect();
        let entry = entry_at(&lines, details.schema_range.start.line as usize)?;
        let refused = lines[entry.dash_line..=entry.end].iter().any(|l| {
            let key = l.trim_start().trim_start_matches("- ");
            ["tests:", "data_tests:", "versions:", "contract:"]
                .iter()
                .any(|k| key.starts_with(k))
        });
        if refused {
            return None;
        }
        yaml_change = Some(DocumentChange::Edit(TextDocumentEdit::new(
            file_uri(schema),
            vec![TextEdit {
                range: lines_range(&text, entry.dash_line, removed_end(&lines, entry.end)),
                new_text: String::new(),
            }],
        )));
    }
    let call = call_span(&parsed.tokens, idx)?;
    let body = indented(sql.trim().trim_end_matches(';').trim_end());
    let insert = cte_insertion(&parsed.tokens, parsed, &name, &body)?;
    let mut edits = vec![insert];
    edits.push(TextEdit {
        range: call,
        new_text: name.clone(),
    });
    let mut changes = vec![DocumentChange::Edit(TextDocumentEdit::new(
        uri.to_string(),
        edits,
    ))];
    changes.extend(yaml_change);
    changes.push(DocumentChange::Delete(DeleteFile::new(file_uri(
        &details.uri,
    ))));
    Some(CodeAction {
        title: format!("Inline model '{name}' as a CTE"),
        kind: Some("refactor.inline"),
        diagnostics: None,
        edit: Some(WorkspaceEdit {
            document_changes: changes,
        }),
        command: None,
        is_preferred: None,
        data: None,
    })
}

/// The `{{ … }}` around the ref token at `idx`, when it holds the call
/// alone.
fn call_span(tokens: &[Token], idx: usize) -> Option<Range> {
    let open = (idx.saturating_sub(4)..idx)
        .rev()
        .find(|&i| tokens[i].kind == TokenKind::JinjaOpen(JinjaKind::Expr))?;
    let close = (idx + 1..(idx + 5).min(tokens.len()))
        .find(|&i| tokens[i].kind == TokenKind::JinjaClose(JinjaKind::Expr))?;
    let (a, b) = (&tokens[open], &tokens[close]);
    Some(Range {
        start: Position {
            line: a.line,
            character: a.column,
        },
        end: Position {
            line: b.line,
            character: b.column + b.len(),
        },
    })
}

/// Where `name as (body)` goes: first among the CTEs of the file's
/// `with`, or a `with` of its own before the first `select`.
fn cte_insertion(
    tokens: &[Token],
    parsed: &cogitator_parser::ParsedDocument,
    name: &str,
    body: &str,
) -> Option<TextEdit> {
    let first = tokens.iter().position(|t| {
        matches!(
            t.kind,
            TokenKind::Word(Keyword::WITH) | TokenKind::Word(Keyword::SELECT)
        )
    })?;
    let t = &tokens[first];
    let at = Position {
        line: t.line,
        character: t.column,
    };
    if t.kind == TokenKind::Word(Keyword::WITH) {
        let next = tokens.get(first + 1)?;
        if parsed.literal(next).eq_ignore_ascii_case("recursive") {
            return None;
        }
        let end = Position {
            line: t.line,
            character: t.column + t.len(),
        };
        return Some(TextEdit {
            range: Range::zero_width(end),
            new_text: format!("\n\n{name} as (\n\n{body}\n\n),"),
        });
    }
    Some(TextEdit {
        range: Range::zero_width(at),
        new_text: format!("with {name} as (\n\n{body}\n\n)\n\n"),
    })
}

/// The entry's last line, with the blank lines after it when another
/// entry follows them (so no gap is left where it stood).
fn removed_end(lines: &[&str], end: usize) -> usize {
    let mut last = end;
    while lines.get(last + 1).is_some_and(|l| l.trim().is_empty()) {
        last += 1;
    }
    if last + 1 < lines.len() {
        last
    } else {
        end
    }
}

fn indented(sql: &str) -> String {
    sql.lines()
        .map(|l| {
            if l.trim().is_empty() {
                String::new()
            } else {
                format!("    {l}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
