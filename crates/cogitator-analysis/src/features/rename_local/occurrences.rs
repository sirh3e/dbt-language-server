//! Where a local name occurs — in its file, and for a macro parameter at
//! the keyword arguments of the macro's calls elsewhere — and the rename
//! built from that: refusals first (an invalid name, a collision, an
//! installed package's macro, a generic test's parameter), then one text
//! edit per occurrence.

use std::collections::BTreeMap;
use std::path::PathBuf;

use cogitator_error::guidance::Guidance;
use cogitator_ir::Span;
use cogitator_lsp::{DocumentChange, Range, TextDocumentEdit, TextEdit, WorkspaceEdit};

use super::jinja::{keyword_args, LocalKind};
use super::names::{offset_of, valid_jinja_name, valid_sql_name};
use super::sql::SqlKind;
use super::{range_of, Target};
use crate::features::references::files::{path_of, project_of_path, Reader};
use crate::features::references::macros::call_sites;
use crate::features::{file_uri, DbtContext, Document};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
    Definition,
    Use,
}

pub(crate) struct Occurrences {
    pub here: Vec<(Span, Role)>,
    pub elsewhere: BTreeMap<PathBuf, Vec<Range>>,
}

/// The occurrences in the target's own file (bindings and uses).
pub(crate) fn here(target: &Target) -> Vec<(Span, Role)> {
    let mut out: Vec<(Span, Role)> = match target {
        Target::Jinja(a, idx) => {
            let l = &a.locals[*idx];
            let defs = l.bindings.iter().map(|s| (*s, Role::Definition));
            defs.chain(l.uses.iter().map(|s| (*s, Role::Use))).collect()
        }
        Target::Sql(s, idx) => {
            let l = &s.locals[*idx];
            std::iter::once((l.definition, Role::Definition))
                .chain(l.uses.iter().map(|s| (*s, Role::Use)))
                .collect()
        }
    };
    out.sort_by_key(|(s, _)| *s);
    out.dedup_by_key(|(s, _)| *s);
    out
}

/// A quoted SQL identifier's name.
pub(crate) fn unquoted(written: &str) -> &str {
    let quoted = written.len() >= 2
        && matches!(
            (written.as_bytes()[0], written.as_bytes()[written.len() - 1]),
            (b'"', b'"') | (b'`', b'`') | (b'[', b']')
        );
    if quoted {
        &written[1..written.len() - 1]
    } else {
        written
    }
}

fn requote(written: &str, new: &str) -> String {
    if unquoted(written).len() == written.len() {
        return new.to_string();
    }
    let (open, close) = (&written[..1], &written[written.len() - 1..]);
    format!("{open}{new}{close}")
}

/// The project a macro parameter's macro belongs to, if it is a
/// registered macro of this file.
fn macro_package(ctx: &DbtContext, uri: &str, name: &str) -> Option<String> {
    let path = path_of(uri)?;
    ctx.macro_detail_map
        .iter()
        .find(|(_, macros)| macros.get(name).is_some_and(|m| m.uri == path))
        .map(|(project, _)| project.clone())
}

/// Whether the local scope of `target` opens with a `{% test %}` tag.
fn in_generic_test(doc: &Document, target: &Target) -> bool {
    let Target::Jinja(a, idx) = target else {
        return false;
    };
    let start = a.scopes[a.locals[*idx].scope].span.start as usize;
    let rest = doc.text.get(start..).unwrap_or_default();
    let rest = rest.strip_prefix("{%").unwrap_or(rest);
    rest.trim_start_matches(['-', '+'])
        .trim_start()
        .starts_with("test")
}

/// The refusals that do not depend on the new name.
pub(crate) fn renamable(
    ctx: &DbtContext,
    doc: &Document,
    uri: &str,
    target: &Target,
) -> Result<(), String> {
    let (a, idx) = match target {
        Target::Jinja(a, idx) => (a, *idx),
        Target::Sql(s, idx) => {
            let local = &s.locals[*idx];
            return match super::sql::missed(&doc.parsed, local, &ctx.dialect) {
                Some(line) => Err(Guidance::new(format!(
                    "`{}` is written on line {} where the SQL binder cannot follow it \
                     (a Jinja branch not taken, or a column name made by Jinja).",
                    local.folded,
                    line + 1
                ))
                .fix("rename it by hand, or simplify the Jinja around it first.")
                .to_message()),
                None => Ok(()),
            };
        }
    };
    let local = &a.locals[idx];
    if local.kind != LocalKind::Param {
        return Ok(());
    }
    if in_generic_test(doc, target) {
        return Err(Guidance::new(format!(
            "`{}` is a parameter of a generic test: its YAML entries pass it by name.",
            local.name
        ))
        .fix("rename it in the test's definition and in every `tests:` entry by hand.")
        .to_message());
    }
    let macro_name = local.macro_name.as_deref().unwrap_or_default();
    match macro_package(ctx, uri, macro_name) {
        Some(pkg) if pkg != ctx.project.name => Err(Guidance::new(format!(
            "`{macro_name}` is defined in the installed package `{pkg}`, not in this project."
        ))
        .fix("rename the parameter in the package's own repository; `dbt deps` would overwrite an edit here.")
        .to_message()),
        _ => Ok(()),
    }
}

/// Every occurrence; with `new`, a call that already passes `new=` is a
/// refusal (it would pass one keyword twice).
pub(crate) fn collect(
    ctx: &DbtContext,
    reader: &Reader<'_>,
    doc: &Document,
    uri: &str,
    target: &Target,
    new: Option<&str>,
) -> Result<Occurrences, String> {
    let mut out = Occurrences {
        here: here(target),
        elsewhere: BTreeMap::new(),
    };
    let Target::Jinja(a, idx) = target else {
        return Ok(out);
    };
    let local = &a.locals[*idx];
    let Some(macro_name) = local.macro_name.as_deref() else {
        return Ok(out);
    };
    let new = new.unwrap_or_default();
    let twice = || {
        format!(
            "a call of `{macro_name}` already passes `{new}=`; renaming `{}` would pass it twice.",
            local.name
        )
    };
    for call in a.calls.iter().filter(|c| c.name == macro_name) {
        let found = keyword_args(&doc.text, call.after_callee as usize, &local.name, new);
        if found.has_new && !found.spans.is_empty() {
            return Err(twice());
        }
        out.here
            .extend(found.spans.into_iter().map(|s| (s, Role::Use)));
    }
    out.here.sort_by_key(|(s, _)| *s);
    out.here.dedup_by_key(|(s, _)| *s);
    let Some(pkg) = macro_package(ctx, uri, macro_name) else {
        return Ok(out); // a template-local macro: its calls are all here
    };
    let current = path_of(uri);
    for site in call_sites(ctx, reader, &pkg, macro_name) {
        if Some(&site.path) == current.as_ref() {
            continue;
        }
        let Some(text) = reader.read(&site.path) else {
            continue;
        };
        for range in &site.ranges {
            let Some(end) = offset_of(&text, range.end) else {
                continue;
            };
            let found = keyword_args(&text, end as usize, &local.name, new);
            if found.has_new && !found.spans.is_empty() {
                return Err(twice());
            }
            let ranges = found.spans.into_iter().map(|s| range_of(&text, s));
            out.elsewhere
                .entry(site.path.clone())
                .or_default()
                .extend(ranges);
        }
    }
    out.elsewhere.retain(|_, r| !r.is_empty());
    Ok(out)
}

fn invalid(new: &str, what: &str) -> String {
    Guidance::new(format!("`{new}` is not a valid {what}."))
        .fix("use letters, digits and underscores, not starting with a digit, and not a keyword.")
        .to_message()
}

fn collided(reason: String) -> String {
    Guidance::new(reason).fix("pick another name.").to_message()
}

pub(crate) fn rename(
    ctx: &DbtContext,
    reader: &Reader<'_>,
    doc: &Document,
    uri: &str,
    target: &Target,
    new: &str,
) -> Result<WorkspaceEdit, String> {
    renamable(ctx, doc, uri, target)?;
    match target {
        Target::Jinja(a, idx) => {
            if !valid_jinja_name(new) {
                return Err(invalid(new, "Jinja variable name"));
            }
            if a.locals[*idx].name != new {
                if let Some(reason) = a.collision(*idx, new) {
                    return Err(collided(reason));
                }
            }
        }
        Target::Sql(s, idx) => {
            let local = &s.locals[*idx];
            let what = match local.kind {
                SqlKind::Cte => "CTE name",
                SqlKind::Alias => "table alias",
            };
            if !valid_sql_name(new) {
                return Err(invalid(new, what));
            }
            let folded = cogitator_sql_front::fold_schema_name(new, &ctx.dialect);
            if local.folded != folded {
                if local.kind == SqlKind::Cte && ctx.model_detail_map.contains_key(new) {
                    return Err(collided(format!(
                        "`{new}` is the name of a model; a CTE so named would be confused with it."
                    )));
                }
                if let Some(reason) = s.collision(*idx, &folded) {
                    return Err(collided(reason));
                }
            }
        }
    }
    let Occurrences { here, elsewhere } = collect(ctx, reader, doc, uri, target, Some(new))?;
    let text = &doc.text;
    let mut changes = vec![DocumentChange::Edit(TextDocumentEdit::new(
        uri.to_string(),
        here.into_iter()
            .map(|(span, _)| TextEdit {
                range: range_of(text, span),
                new_text: requote(&text[span.start as usize..span.end as usize], new),
            })
            .collect(),
    ))];
    for (path, ranges) in elsewhere {
        // installed packages' files are dbt deps', never edited
        if project_of_path(ctx, &path) != ctx.project.name {
            continue;
        }
        let edits = ranges
            .into_iter()
            .map(|range| TextEdit {
                range,
                new_text: new.to_string(),
            })
            .collect();
        changes.push(DocumentChange::Edit(TextDocumentEdit::new(
            file_uri(&path),
            edits,
        )));
    }
    Ok(WorkspaceEdit {
        document_changes: changes,
    })
}
