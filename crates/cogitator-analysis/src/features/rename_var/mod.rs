//! textDocument/rename for vars: from a `var('x')` call or from a `vars:`
//! key (a dbt_project.yml's — root, scoped or a package's — or a
//! cogitator.yml override), rename every call that resolves to the same
//! definition and the keys that decide it, as one WorkspaceEdit.
//!
//! Scoping rule: the rename follows the *definition* D the starting
//! point resolves to. Calls are renamed in the scopes (the root project,
//! each installed package) whose `var('x')` resolves to D. A key of the
//! same name moves with them only when every scope that can see it
//! resolves to D — so renaming a package-scoped `vars.utils.x` leaves
//! the root's global `x` (which other scopes still read) alone, while
//! renaming an override renames every same-name key it shadows.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cogitator_lsp::{
    DocumentChange, Position, PrepareRenameResult, Range, TextDocumentEdit, TextEdit, WorkspaceEdit,
};
use cogitator_parser::{DocumentParser, JinjaSqlParser, TokenKind};

use super::vars::{contains, scope_vars, var_files, var_key_at};
use super::{file_uri, token_range, DbtContext, Document};
use crate::context::VarSource;

mod plan;

use plan::{plan, Entry, Scope, Start};

fn all_scopes(ctx: &DbtContext) -> Vec<Scope> {
    std::iter::once(None)
        .chain(ctx.package_variables.iter().map(|p| Some(p.name.clone())))
        .collect()
}

/// Every key that defines `name` anywhere, with its visibility.
fn entries_of(ctx: &DbtContext, name: &str) -> Vec<Entry> {
    let Some(root) = ctx.project_root.as_deref() else {
        return Vec::new();
    };
    let scopes = all_scopes(ctx);
    let scope_named = |block: &str| -> Option<Scope> {
        if block == ctx.project.name {
            Some(None)
        } else {
            ctx.package_variables
                .iter()
                .any(|p| p.name == block)
                .then(|| Some(block.to_string()))
        }
    };
    let mut out = Vec::new();
    let root_file = root.join("dbt_project.yml");
    let is_block = |key: &str, value: Option<&serde_json::Value>| {
        value.is_some_and(|v| v.is_object()) && scope_named(key).is_some()
    };
    if let Some(r) = ctx.project.var_ranges.get(name) {
        if !is_block(name, ctx.project.vars.get(name)) {
            out.push(Entry {
                path: root_file.clone(),
                range: *r,
                visible: scopes.clone(),
            });
        }
    }
    for (block, keys) in &ctx.project.nested_var_ranges {
        if let (Some(scope), Some(r)) = (scope_named(block), keys.get(name)) {
            out.push(Entry {
                path: root_file.clone(),
                range: *r,
                visible: vec![scope],
            });
        }
    }
    for p in &ctx.package_variables {
        let file = crate::context::load_project_file_at(&p.root);
        let path = p.root.join("dbt_project.yml");
        let own = Some(p.name.clone());
        if let Some(r) = file.var_ranges.get(name) {
            if !is_block(name, file.vars.get(name)) {
                out.push(Entry {
                    path: path.clone(),
                    range: *r,
                    visible: vec![own.clone()],
                });
            }
        }
        if let Some(r) = file
            .nested_var_ranges
            .get(&p.name)
            .and_then(|m| m.get(name))
        {
            out.push(Entry {
                path,
                range: *r,
                visible: vec![own],
            });
        }
    }
    if let Some(v) = ctx.variable_detail_map.get(name) {
        if v.source == VarSource::Override && !v.uri.as_os_str().is_empty() {
            out.push(Entry {
                path: v.uri.clone(),
                range: v.range,
                visible: scopes,
            });
        }
    }
    out
}

fn winners(ctx: &DbtContext, name: &str, entries: &[Entry]) -> BTreeMap<Scope, usize> {
    all_scopes(ctx)
        .into_iter()
        .filter_map(|s| {
            let v = scope_vars(ctx, s.as_deref()).get(name)?;
            let i = entries
                .iter()
                .position(|e| e.path == v.uri && e.range == v.range)?;
            Some((s, i))
        })
        .collect()
}

/// The var under the cursor: its name and where the rename starts.
fn start_at(
    ctx: &DbtContext,
    doc: Option<&Document>,
    uri: &str,
    pos: Position,
) -> Option<(String, Range, Vec<Entry>, Start)> {
    let call = doc.and_then(|d| {
        let idx = d.parsed.find_token_at_cursor(pos.line, pos.character)?;
        let t = &d.parsed.tokens[idx];
        (t.kind == TokenKind::Var).then(|| (d.parsed.literal(t).to_string(), token_range(t)))
    });
    if let Some((name, range)) = call {
        let entries = entries_of(ctx, &name);
        let start = match ctx.var_for(uri, &name) {
            Some(v) => Start::Definition(
                entries
                    .iter()
                    .position(|e| e.path == v.uri && e.range == v.range)?,
            ),
            None => Start::Undefined(ctx.package_of_uri(uri).map(|p| p.name.clone())),
        };
        return Some((name, range, entries, start));
    }
    let path = PathBuf::from(cogitator_lsp::uri_to_fs_path(uri)?);
    let name = match var_key_at(ctx, uri, pos) {
        Some((name, _)) => name,
        None => ctx
            .variable_detail_map
            .values()
            .find(|v| v.source == VarSource::Override && v.uri == path && contains(&v.range, pos))?
            .name
            .clone(),
    };
    let entries = entries_of(ctx, &name);
    let k = entries
        .iter()
        .position(|e| e.path == path && contains(&e.range, pos))?;
    Some((name, entries[k].range, entries.clone(), Start::Key(k)))
}

/// A key's range without its quotes (`'x':` → `x`), from its file text.
fn key_name_range(text: Option<&str>, range: Range, name: &str) -> Range {
    let quoted = range.end.character - range.start.character == name.len() as u32 + 2;
    let line = text.and_then(|t| t.lines().nth(range.start.line as usize));
    let starts_quoted = line
        .and_then(|l| l.get(range.start.character as usize..))
        .is_some_and(|rest| rest.starts_with(['\'', '"']));
    if quoted || starts_quoted {
        Range {
            start: Position {
                line: range.start.line,
                character: range.start.character + 1,
            },
            end: Position {
                line: range.start.line,
                character: range.start.character + 1 + name.len() as u32,
            },
        }
    } else {
        range
    }
}

/// textDocument/prepareRename on a var call or key.
pub fn prepare_rename_var(
    ctx: &DbtContext,
    doc: Option<&Document>,
    uri: &str,
    pos: Position,
) -> Option<PrepareRenameResult> {
    let (name, range, _, start) = start_at(ctx, doc, uri, pos)?;
    let range = match start {
        Start::Key(_) => key_name_range(doc.map(|d| &*d.text), range, &name),
        _ => range,
    };
    Some(PrepareRenameResult {
        range,
        placeholder: name,
    })
}

/// The rename dispatcher's var step: `None` when the cursor is on no
/// var (the macro and model steps follow), else the edit or why the
/// rename is refused.
pub fn rename_var_at(
    ctx: &DbtContext,
    doc: Option<&Document>,
    uri: &str,
    pos: Position,
    new_name: &str,
    read: &dyn Fn(&Path) -> Option<String>,
) -> Option<Result<WorkspaceEdit, String>> {
    let (name, ..) = start_at(ctx, doc, uri, pos)?;
    Some(
        rename_var(ctx, doc, uri, pos, new_name, read).ok_or_else(|| {
            format!(
                "cannot rename var '{name}' to '{new_name}': the new name must be a plain \
                 identifier that no renamed scope already sees"
            )
        }),
    )
}

/// textDocument/rename on a var call or key (`None`: not a var, or the
/// new name is not a plain identifier or already taken in scope).
/// `read` reads a file other than the open document.
pub fn rename_var(
    ctx: &DbtContext,
    doc: Option<&Document>,
    uri: &str,
    pos: Position,
    new_name: &str,
    read: &dyn Fn(&Path) -> Option<String>,
) -> Option<WorkspaceEdit> {
    let (name, _, entries, start) = start_at(ctx, doc, uri, pos)?;
    if new_name.is_empty()
        || new_name == name
        || !new_name.chars().all(|c| c.is_alphanumeric() || c == '_')
    {
        return None;
    }
    let winners = winners(ctx, &name, &entries);
    let plan = plan(&entries, &winners, start);
    // never merge into a var a renamed scope already sees
    let touched: Vec<&Scope> = plan
        .scopes
        .iter()
        .chain(plan.keys.iter().flat_map(|&k| entries[k].visible.iter()))
        .collect();
    if touched
        .iter()
        .any(|s| scope_vars(ctx, s.as_deref()).contains_key(new_name))
    {
        return None;
    }
    let text_of = |path: &Path| -> Option<String> {
        match doc.filter(|_| file_uri(path) == uri) {
            Some(d) => Some(d.text.to_string()),
            None => read(path),
        }
    };
    let mut edits: BTreeMap<PathBuf, Vec<TextEdit>> = BTreeMap::new();
    for &k in &plan.keys {
        let e = &entries[k];
        let range = key_name_range(text_of(&e.path).as_deref(), e.range, &name);
        edits.entry(e.path.clone()).or_default().push(TextEdit {
            range,
            new_text: new_name.to_string(),
        });
    }
    let mut files = var_files(ctx);
    if let Some(root) = ctx.project_root.as_deref() {
        files.push(root.join("dbt_project.yml"));
    }
    files.extend(
        ctx.package_variables
            .iter()
            .map(|p| p.root.join("dbt_project.yml")),
    );
    files.sort();
    files.dedup();
    for path in files {
        let furi = file_uri(&path);
        let scope = ctx.package_of_uri(&furi).map(|p| p.name.clone());
        if !plan.scopes.contains(&scope) {
            continue;
        }
        let Some(text) = text_of(&path) else { continue };
        if !text.contains(name.as_str()) {
            continue;
        }
        let Ok(parsed) = JinjaSqlParser.parse(&text, &ctx.dialect) else {
            continue;
        };
        let calls = parsed
            .tokens
            .iter()
            .filter(|t| t.kind == TokenKind::Var && parsed.literal(t) == name)
            .map(|t| TextEdit {
                range: token_range(t),
                new_text: new_name.to_string(),
            });
        let list = edits.entry(path.clone()).or_default();
        for edit in calls {
            if !list.iter().any(|e| e.range == edit.range) {
                list.push(edit);
            }
        }
    }
    let document_changes = edits
        .into_iter()
        .filter(|(_, e)| !e.is_empty())
        .map(|(path, e)| DocumentChange::Edit(TextDocumentEdit::new(file_uri(&path), e)))
        .collect();
    Some(WorkspaceEdit { document_changes })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_keys_lose_their_quotes() {
        let r = |a, b| Range {
            start: Position {
                line: 0,
                character: a,
            },
            end: Position {
                line: 0,
                character: b,
            },
        };
        assert_eq!(key_name_range(Some("  'ab': 1"), r(2, 6), "ab"), r(3, 5));
        assert_eq!(key_name_range(Some("  ab: 1"), r(2, 4), "ab"), r(2, 4));
    }
}
