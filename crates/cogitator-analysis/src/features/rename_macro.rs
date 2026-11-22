//! textDocument/rename for macros, and the prepareRename that admits
//! them: the definition header, every call resolving to the macro
//! project-wide (bare and `pkg.name` alike, in models, tests, analyses
//! and macro files) and the properties-YAML `macros:` entries — one
//! WorkspaceEdit, one text edit per file. A macro of an installed package
//! is refused: `dbt deps` would overwrite the edit.
//!
//! [`rename_at`] / [`prepare_rename_at`] are the entry points: a macro
//! target is handled here, everything else goes to the model rename.

use std::collections::BTreeMap;
use std::path::PathBuf;

use cogitator_error::guidance::Guidance;
use cogitator_lsp::{
    DocumentChange, Position, PrepareRenameResult, Range, TextDocumentEdit, TextEdit, WorkspaceEdit,
};
use cogitator_parser::TokenKind;

use super::references::files::{project_of_path, yaml_files, Reader};
use super::references::macros::{call_sites, definition_range};
use super::references::occur::{contains, macro_def_at, macro_name_range};
use super::references::target::{target_at, Target};
use super::references::yaml_calls::macro_entries;
use super::{file_uri, token_range, DbtContext, Document};

/// prepareRename: a macro (call, header or YAML entry) or a `ref()`.
/// `Err` carries the user-facing refusal (an installed package's macro).
pub fn prepare_rename_at(
    ctx: &DbtContext,
    doc: &Document,
    uri: &str,
    pos: Position,
) -> Result<Option<PrepareRenameResult>, String> {
    let Some(Target::Macro { package, name }) = target_at(ctx, doc, uri, pos) else {
        return Ok(super::prepare_rename(doc, pos));
    };
    refuse_package(ctx, &package, &name)?;
    Ok(
        name_range_at(doc, pos, &name).map(|range| PrepareRenameResult {
            range,
            placeholder: name,
        }),
    )
}

/// rename: a macro target renames the macro, anything else the model.
pub fn rename_at(
    ctx: &DbtContext,
    doc: Option<&Document>,
    uri: &str,
    pos: Position,
    new_name: &str,
) -> Result<Option<WorkspaceEdit>, String> {
    match doc.and_then(|d| target_at(ctx, d, uri, pos)) {
        Some(Target::Macro { package, name }) => {
            let reader = Reader::disk(doc.map(|d| (uri, &*d.text)));
            rename_macro(ctx, &reader, &package, &name, new_name).map(Some)
        }
        _ => Ok(super::rename_model(ctx, doc, uri, pos, new_name)),
    }
}

/// The range of the macro's name under the cursor.
fn name_range_at(doc: &Document, pos: Position, name: &str) -> Option<Range> {
    let parsed = &doc.parsed;
    if let Some(idx) = parsed.find_token_at_cursor(pos.line, pos.character) {
        let t = &parsed.tokens[idx];
        if t.kind == TokenKind::Macro {
            return Some(token_range(t));
        }
    }
    if let Some(raw) = macro_def_at(&doc.text, pos) {
        return Some(macro_name_range(&raw));
    }
    macro_entries(&doc.text, name)
        .into_iter()
        .find(|r| contains(r, pos))
}

fn refuse_package(ctx: &DbtContext, package: &str, name: &str) -> Result<(), String> {
    if package == ctx.project.name {
        return Ok(());
    }
    Err(Guidance::new(format!(
        "`{name}` is defined in the installed package `{package}`, not in this project."
    ))
    .fix(format!(
        "rename it in `{package}`'s own repository; `dbt deps` would overwrite an edit here. \
         To change it locally, copy the macro into this project's macro-paths under a new name."
    ))
    .to_message())
}

pub(crate) fn valid_macro_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub(crate) fn rename_macro(
    ctx: &DbtContext,
    reader: &Reader<'_>,
    package: &str,
    name: &str,
    new_name: &str,
) -> Result<WorkspaceEdit, String> {
    refuse_package(ctx, package, name)?;
    if !valid_macro_name(new_name) {
        return Err(
            Guidance::new(format!("`{new_name}` is not a valid macro name."))
                .fix("use letters, digits and underscores, not starting with a digit.")
                .to_message(),
        );
    }
    if new_name != name
        && ctx
            .macro_detail_map
            .get(package)
            .is_some_and(|m| m.contains_key(new_name))
    {
        return Err(Guidance::new(format!(
            "a macro named `{new_name}` already exists in `{package}`."
        ))
        .fix("pick another name, or rename or remove the existing macro first.")
        .to_message());
    }

    let mut edits: BTreeMap<PathBuf, Vec<Range>> = BTreeMap::new();
    if let Some((path, range)) = definition_range(ctx, package, name) {
        edits.entry(path).or_default().push(range);
    }
    // installed packages' own files are never edited (dbt deps owns them)
    for site in call_sites(ctx, reader, package, name) {
        if project_of_path(ctx, &site.path) != ctx.project.name {
            continue;
        }
        edits.entry(site.path).or_default().extend(site.ranges);
    }
    for path in yaml_files(ctx) {
        let Some(text) = reader.read(&path) else {
            continue;
        };
        let ranges = macro_entries(&text, name);
        if !ranges.is_empty() {
            edits.entry(path).or_default().extend(ranges);
        }
    }

    let document_changes = edits
        .into_iter()
        .map(|(path, mut ranges)| {
            ranges.sort_by_key(|r| (r.start.line, r.start.character));
            ranges.dedup();
            let edits = ranges
                .into_iter()
                .map(|range| TextEdit {
                    range,
                    new_text: new_name.to_string(),
                })
                .collect();
            DocumentChange::Edit(TextDocumentEdit::new(file_uri(&path), edits))
        })
        .collect();
    Ok(WorkspaceEdit { document_changes })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macro_names() {
        for (name, ok) in [
            ("full_name", true),
            ("_private", true),
            ("v2", true),
            ("2fast", false),
            ("", false),
            ("bad name", false),
            ("pkg.name", false),
            ("naïve", false),
        ] {
            assert_eq!(valid_macro_name(name), ok, "{name}");
        }
    }
}
