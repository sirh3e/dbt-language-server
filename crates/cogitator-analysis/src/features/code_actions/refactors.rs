//! Small refactors at the cursor: expand a star to the columns the
//! project knows (`refactor.rewrite`), and the schema-YAML extraction
//! of `dbt.extractModelYaml` as an edit (`refactor.extract`), for
//! editors that run no custom commands.

use cogitator_lsp::{
    CodeAction, DocumentChange, Position, Range, TextDocumentEdit, TextEdit, WorkspaceEdit,
};
use cogitator_parser::{Keyword, TokenKind};

use super::super::completion::scope::{relation_columns, RelationTarget, ScopeIndex};
use super::super::extract::ExtractEdit;
use super::super::{DbtContext, Document};

/// On `c.*` (cursor on any of its three tokens): `c.a, c.b, …` with the
/// columns `c` is known to have. On a bare `*` of a query reading one
/// relation: that relation's columns, unqualified. None where the
/// columns are not known — an expansion must not drop any.
pub fn expand_star_action(
    ctx: &DbtContext,
    doc: &Document,
    uri: &str,
    range: Range,
) -> Option<CodeAction> {
    let parsed = &doc.parsed;
    let tokens = &parsed.tokens;
    let at = parsed.find_token_at_cursor(range.start.line, range.start.character)?;
    // the star of the expression under the cursor
    let star =
        (at..tokens.len().min(at + 3)).find(|&i| tokens[i].kind == TokenKind::Symbol('*'))?;
    let qualified = star >= 2 && tokens[star - 1].kind == TokenKind::Symbol('.');
    // the query the star selects from: the parentheses around it (a CTE
    // body, a subquery), else the whole document
    let (lo, hi) = enclosing_query(tokens, star);
    let local = ScopeIndex::build_within(doc, lo, hi);
    let relations = distinct(&local);
    let (start, alias, target) = if qualified {
        let alias = parsed.literal(&tokens[star - 2]);
        let target = match local.resolve(alias) {
            Some(target) => target.clone(),
            None => ScopeIndex::build(doc).resolve(alias)?.clone(),
        };
        let target = &target;
        (star - 2, Some(alias), target.clone())
    } else {
        let [target] = relations.as_slice() else {
            return None;
        };
        (star, None, (*target).clone())
    };
    if start > at {
        return None; // the cursor is before the expression, not on it
    }
    // a select item, not `count(*)` or a product: SELECT, DISTINCT or a
    // comma stands right before it
    let item_start = start.checked_sub(1).map(|i| tokens[i].kind);
    if !matches!(
        item_start,
        Some(TokenKind::Word(Keyword::SELECT))
            | Some(TokenKind::Word(Keyword::DISTINCT))
            | Some(TokenKind::Symbol(','))
    ) {
        return None;
    }
    let columns = relation_columns(ctx, doc, &target);
    if columns.is_empty() {
        return None;
    }
    let new_text = columns
        .iter()
        .map(|c| match alias {
            Some(a) => format!("{a}.{c}"),
            None => c.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ");
    let (first, last) = (&tokens[start], &tokens[star]);
    let range = Range {
        start: Position {
            line: first.line,
            character: first.column,
        },
        end: Position {
            line: last.line,
            character: last.column + last.len(),
        },
    };
    let shown = alias.map_or("*".to_string(), |a| format!("{a}.*"));
    Some(CodeAction {
        title: format!("Expand {shown} to {} column(s)", columns.len()),
        kind: Some("refactor.rewrite"),
        diagnostics: None,
        edit: Some(WorkspaceEdit {
            document_changes: vec![DocumentChange::Edit(TextDocumentEdit::new(
                uri.to_string(),
                vec![TextEdit { range, new_text }],
            ))],
        }),
        command: None,
        is_preferred: None,
        data: None,
    })
}

/// The token bounds of the query around `at`: just inside the nearest
/// unmatched `(` before it and its `)` after it, or the whole document
/// where no parenthesis encloses it.
fn enclosing_query(tokens: &[cogitator_parser::Token], at: usize) -> (usize, usize) {
    let mut depth = 0usize;
    let mut lo = 0;
    for i in (0..at).rev() {
        match tokens[i].kind {
            TokenKind::Symbol(')') => depth += 1,
            TokenKind::Symbol('(') if depth == 0 => {
                lo = i + 1;
                break;
            }
            TokenKind::Symbol('(') => depth -= 1,
            _ => {}
        }
    }
    if lo == 0 {
        return (0, tokens.len());
    }
    let mut depth = 0usize;
    for (i, token) in tokens.iter().enumerate().skip(at) {
        match token.kind {
            TokenKind::Symbol('(') => depth += 1,
            TokenKind::Symbol(')') if depth == 0 => return (lo, i),
            TokenKind::Symbol(')') => depth -= 1,
            _ => {}
        }
    }
    (lo, tokens.len())
}

/// Each relation of the scope once (it lists an aliased one twice).
fn distinct(scope: &ScopeIndex) -> Vec<&RelationTarget> {
    let mut out: Vec<&RelationTarget> = Vec::new();
    for target in scope.targets() {
        if !out.contains(&target) {
            out.push(target);
        }
    }
    out
}

/// The schema-YAML extraction as an edit-bearing action: a new
/// `<model>.yml`, or the next version of a documented model.
pub fn extract_yaml_action(model: &str, extract: &ExtractEdit) -> CodeAction {
    let title = match extract {
        ExtractEdit::CreateFile { path, .. } => format!(
            "Extract schema YAML of '{model}' to {}",
            path.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        ),
        ExtractEdit::AddVersion { next, .. } => format!("Add version {next} of '{model}'"),
    };
    CodeAction {
        title,
        kind: Some("refactor.extract"),
        diagnostics: None,
        edit: Some(extract.workspace_edit()),
        command: None,
        is_preferred: None,
        data: None,
    }
}
