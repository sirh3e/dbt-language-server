//! Var navigation beyond the call site: every `var('x')` that resolves
//! to the same definition (references), and the keys of `vars:` in a
//! dbt_project.yml as a starting point for both.

use std::path::{Path, PathBuf};

use cogitator_lsp::{Location, Position, Range};
use cogitator_parser::{DocumentParser, JinjaSqlParser, TokenKind};

use super::{file_uri, token_range, DbtContext, Document};
use crate::context::VariableDetails;

/// A `vars:` key under the cursor in a dbt_project.yml (the root's or a
/// package's): the var's name and the scope whose files see it.
pub fn var_key_at(ctx: &DbtContext, uri: &str, pos: Position) -> Option<(String, Option<String>)> {
    let path = PathBuf::from(cogitator_lsp::uri_to_fs_path(uri)?);
    if path.file_name()? != "dbt_project.yml" {
        return None;
    }
    let root = ctx.project_root.as_deref()?;
    let file = if path == root.join("dbt_project.yml") {
        &ctx.project
    } else {
        let pkg = ctx.package_of_uri(uri)?;
        return key_in(&project_of(ctx, &pkg.name)?, pos).map(|(n, _)| (n, Some(pkg.name.clone())));
    };
    key_in(file, pos).map(|(name, block)| {
        // a key inside `vars: {<package>: …}` is that package's var
        let scope = block.filter(|b| b != &ctx.project.name);
        (name, scope)
    })
}

fn project_of(ctx: &DbtContext, package: &str) -> Option<cogitator_schemas::project::ProjectFile> {
    let pkg = ctx.package_variables.iter().find(|p| p.name == package)?;
    Some(crate::context::load_project_file_at(&pkg.root))
}

/// The var key at `pos` and, for a nested key, its block's name.
fn key_in(
    file: &cogitator_schemas::project::ProjectFile,
    pos: Position,
) -> Option<(String, Option<String>)> {
    for (block, keys) in &file.nested_var_ranges {
        for (name, range) in keys {
            if contains(range, pos) {
                return Some((name.clone(), Some(block.clone())));
            }
        }
    }
    file.var_ranges
        .iter()
        .find(|(_, range)| contains(range, pos))
        .map(|(name, _)| (name.clone(), None))
}

fn contains(range: &Range, pos: Position) -> bool {
    range.start.line == pos.line
        && range.start.character <= pos.character
        && pos.character <= range.end.character
}

/// The vars a scope's files see (`None`: the root project's).
fn scope_vars<'a>(
    ctx: &'a DbtContext,
    scope: Option<&str>,
) -> &'a std::collections::HashMap<String, VariableDetails> {
    scope
        .and_then(|s| ctx.package_variables.iter().find(|p| p.name == s))
        .map(|p| &p.vars)
        .unwrap_or(&ctx.variable_detail_map)
}

/// Definition from a `vars:` key: the entry that decides the value its
/// scope sees (an override in cogitator.yml, say), when that is
/// another entry than the key itself.
pub fn key_definition(ctx: &DbtContext, uri: &str, pos: Position) -> Option<Location> {
    let (name, scope) = var_key_at(ctx, uri, pos)?;
    let var = scope_vars(ctx, scope.as_deref()).get(&name)?;
    if var.uri.as_os_str().is_empty() {
        return None;
    }
    Some(Location {
        uri: file_uri(&var.uri),
        range: var.range,
    })
}

/// References of a var (`None` when the cursor is on no var): every
/// `var('name')` in the project's and packages' SQL and YAML files that
/// resolves to the same definition, and that definition. `doc` is the
/// open document at `uri`, read in place of its file.
pub fn var_references(
    ctx: &DbtContext,
    doc: Option<&Document>,
    uri: &str,
    pos: Position,
    read: &dyn Fn(&Path) -> Option<String>,
) -> Option<Vec<Location>> {
    let at_call = doc.and_then(|d| {
        let idx = d.parsed.find_token_at_cursor(pos.line, pos.character)?;
        let t = &d.parsed.tokens[idx];
        (t.kind == TokenKind::Var).then(|| d.parsed.literal(t).to_string())
    });
    // the definition every usage must resolve to (None: undefined)
    let (name, target) = match at_call {
        Some(name) => {
            let target = ctx.var_for(uri, &name).cloned();
            (name, Some(target))
        }
        // from a key: every usage of the name, whichever scope
        None => (var_key_at(ctx, uri, pos)?.0, None),
    };
    let same = |file_uri: &str| match &target {
        None => true,
        Some(t) => {
            let seen = ctx.var_for(file_uri, &name);
            match (t, seen) {
                (None, None) => true,
                (Some(a), Some(b)) => a.uri == b.uri && a.range == b.range,
                _ => false,
            }
        }
    };

    let mut out = Vec::new();
    for path in var_files(ctx) {
        let file = file_uri(&path);
        let text = match doc.filter(|_| file == uri) {
            Some(d) => d.text.to_string(),
            None => match read(&path) {
                Some(t) => t,
                None => continue,
            },
        };
        if !text.contains(name.as_str()) || !same(&file) {
            continue;
        }
        let Ok(parsed) = JinjaSqlParser.parse(&text, &ctx.dialect) else {
            continue;
        };
        out.extend(
            parsed
                .tokens
                .iter()
                .filter(|t| t.kind == TokenKind::Var && parsed.literal(t) == name)
                .map(|t| Location {
                    uri: file.clone(),
                    range: token_range(t),
                }),
        );
    }
    // the definition(s) too
    let definitions: Vec<&VariableDetails> = match &target {
        Some(Some(t)) => vec![t],
        Some(None) => Vec::new(),
        None => std::iter::once(&ctx.variable_detail_map)
            .chain(ctx.package_variables.iter().map(|p| &p.vars))
            .filter_map(|m| m.get(&name))
            .collect(),
    };
    for d in definitions {
        if d.uri.as_os_str().is_empty() {
            continue;
        }
        let loc = Location {
            uri: file_uri(&d.uri),
            range: d.range,
        };
        if !out.contains(&loc) {
            out.push(loc);
        }
    }
    Some(out)
}

/// Every SQL and YAML file a `var()` may be written in: the resource
/// paths of the root project and of each installed package.
fn var_files(ctx: &DbtContext) -> Vec<PathBuf> {
    let Some(root) = ctx.project_root.as_deref() else {
        return Vec::new();
    };
    let mut roots: Vec<(PathBuf, cogitator_schemas::project::ProjectFile)> =
        vec![(root.to_path_buf(), ctx.project.clone())];
    for p in &ctx.package_variables {
        roots.push((
            p.root.clone(),
            crate::context::load_project_file_at(&p.root),
        ));
    }
    let mut files = Vec::new();
    for (base, project) in &roots {
        let dirs = project
            .model_paths
            .iter()
            .chain(&project.macro_paths)
            .chain(&project.test_paths)
            .chain(&project.snapshot_paths)
            .chain(&project.analysis_paths)
            .chain(&project.seed_paths);
        for dir in dirs {
            let dir = base.join(dir);
            for ext in [".sql", ".yml", ".yaml"] {
                files.extend(crate::util::walk_filepath(&dir, ext));
            }
        }
    }
    files.sort();
    files.dedup();
    files
}
