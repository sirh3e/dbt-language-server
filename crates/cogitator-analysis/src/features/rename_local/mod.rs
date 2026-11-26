//! Rename, references, document highlights and prepareRename for names
//! local to one file: Jinja locals (`set`, block `set`, `for` targets,
//! macro and `call(…)` parameters; [`jinja`]) and SQL names of a model
//! (CTEs and table aliases; [`sql`]). Each entry point answers `None`
//! when the cursor is on no such name, so the dispatcher falls through to
//! the var, macro and model features; a refusal is `Some(Err(reason))`.
//!
//! A macro parameter is part of the macro's API: renaming it also renames
//! the keyword arguments `m(b=2)` of every call of the macro, project-wide
//! (installed packages' files are never edited). Positional arguments do
//! not name the parameter and are left alone.

pub(crate) mod jinja;
mod names;
mod occurrences;
pub(crate) mod sql;

#[cfg(test)]
mod tests;

use cogitator_ir::Span;
use cogitator_lsp::{DocumentHighlight, Location, Position, PrepareRenameResult, WorkspaceEdit};

use super::references::files::{path_of, Reader};
use super::{byte_position, DbtContext, Document};
use occurrences::{Occurrences, Role};

/// What the cursor names.
pub(crate) enum Target {
    Jinja(jinja::Analysis, usize),
    Sql(sql::SqlLocals, usize),
}

fn is_macro_file(ctx: &DbtContext, uri: &str) -> bool {
    let Some(path) = path_of(uri) else {
        return false;
    };
    ctx.macro_detail_map
        .values()
        .any(|macros| macros.values().any(|m| m.uri == path))
}

fn is_sql_file(uri: &str) -> bool {
    path_of(uri).is_some_and(|p| p.extension().is_some_and(|e| e == "sql"))
}

fn target_at(
    ctx: &DbtContext,
    doc: &Document,
    uri: &str,
    pos: Position,
) -> Option<Result<Target, String>> {
    if !is_sql_file(uri) {
        return None;
    }
    let offset = names::offset_of(&doc.text, pos)?;
    if let Some(analysis) = jinja::analyze(&doc.text) {
        if let Some(idx) = analysis.at(offset) {
            return Some(Ok(Target::Jinja(analysis, idx)));
        }
    }
    if is_macro_file(ctx, uri) {
        return None;
    }
    match sql::sql_locals(&doc.text, &ctx.dialect) {
        Ok(locals) => locals.at(offset).map(|idx| Ok(Target::Sql(locals, idx))),
        // a CTE name the binder could not read is refused, not guessed
        Err(reason) => {
            let idx = doc.parsed.find_token_at_cursor(pos.line, pos.character)?;
            let literal = doc.parsed.literal(&doc.parsed.tokens[idx]);
            doc.parsed
                .def_tokens
                .contains_key(literal)
                .then_some(Err(reason))
        }
    }
}

fn range_of(text: &str, span: Span) -> cogitator_lsp::Range {
    cogitator_lsp::Range {
        start: byte_position(text, span.start),
        end: byte_position(text, span.end),
    }
}

/// prepareRename on a local name: its occurrence under the cursor.
pub fn prepare_rename_local(
    ctx: &DbtContext,
    doc: Option<&Document>,
    uri: &str,
    pos: Position,
) -> Option<Result<PrepareRenameResult, String>> {
    let doc = doc?;
    let target = match target_at(ctx, doc, uri, pos)? {
        Ok(target) => target,
        Err(reason) => return Some(Err(reason)),
    };
    if let Err(reason) = occurrences::renamable(ctx, doc, uri, &target) {
        return Some(Err(reason));
    }
    let offset = names::offset_of(&doc.text, pos)?;
    let span = occurrences::here(&target)
        .into_iter()
        .map(|(s, _)| s)
        .find(|s| s.start <= offset && offset <= s.end)?;
    let written = &doc.text[span.start as usize..span.end as usize];
    Some(Ok(PrepareRenameResult {
        range: range_of(&doc.text, span),
        placeholder: occurrences::unquoted(written).to_string(),
    }))
}

/// textDocument/rename on a local name.
pub fn rename_local_at(
    ctx: &DbtContext,
    doc: Option<&Document>,
    uri: &str,
    pos: Position,
    new_name: &str,
    read: &dyn Fn(&std::path::Path) -> Option<String>,
) -> Option<Result<WorkspaceEdit, String>> {
    let doc = doc?;
    let target = match target_at(ctx, doc, uri, pos)? {
        Ok(target) => target,
        Err(reason) => return Some(Err(reason)),
    };
    let reader = Reader::with(Some((uri, &doc.text)), read);
    Some(occurrences::rename(
        ctx, &reader, doc, uri, &target, new_name,
    ))
}

/// textDocument/references on a local name.
pub fn local_references(
    ctx: &DbtContext,
    doc: Option<&Document>,
    uri: &str,
    pos: Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    let doc = doc?;
    let target = target_at(ctx, doc, uri, pos)?.ok()?;
    let reader = Reader::disk(Some((uri, &doc.text)));
    let Occurrences { here, elsewhere } =
        occurrences::collect(ctx, &reader, doc, uri, &target, None).ok()?;
    let mut out: Vec<Location> = here
        .into_iter()
        .filter(|(_, role)| include_declaration || *role == Role::Use)
        .map(|(span, _)| Location {
            uri: uri.to_string(),
            range: range_of(&doc.text, span),
        })
        .collect();
    for (path, ranges) in elsewhere {
        let uri = super::file_uri(&path);
        out.extend(ranges.into_iter().map(|range| Location {
            uri: uri.clone(),
            range,
        }));
    }
    Some(out)
}

/// textDocument/documentHighlight on a local name: bindings are writes,
/// uses reads.
pub fn local_highlights(
    ctx: &DbtContext,
    doc: Option<&Document>,
    uri: &str,
    pos: Position,
) -> Option<Vec<DocumentHighlight>> {
    let doc = doc?;
    let target = target_at(ctx, doc, uri, pos)?.ok()?;
    Some(
        occurrences::here(&target)
            .into_iter()
            .map(|(span, role)| DocumentHighlight {
                range: range_of(&doc.text, span),
                kind: match role {
                    Role::Definition => 3,
                    Role::Use => 2,
                },
            })
            .collect(),
    )
}
