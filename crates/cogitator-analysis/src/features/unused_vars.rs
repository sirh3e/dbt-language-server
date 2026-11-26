//! `unused-var`: a key of the root dbt_project.yml's `vars:` that no
//! file of the scopes that see it reads via `var()` (hint severity,
//! tagged Unnecessary), with a quickfix that removes the entry.
//!
//! Scope-aware: a top-level key counts as read when any project — the
//! root or an installed package — reads it; `vars.<root>.x` when the
//! root project does; `vars.<package>.x` when that package does. Only
//! the root dbt_project.yml is checked (packages are installed code),
//! and cogitator.yml overrides are not: they also feed the dbt commands
//! the server runs, where a dynamic `var(name)` the scan cannot follow
//! may read them. A top-level mapping whose key names a package that is
//! declared but not installed is a scope block waiting for `dbt deps`,
//! and is skipped. Reads by a dynamic name (`var(x)`) are invisible.

use std::collections::HashSet;
use std::path::Path;

use cogitator_error::guidance::Guidance;
use cogitator_lsp::{
    diagnostic_severity, diagnostic_tag, Diagnostic, DiagnosticData, FixKind, MultiEditFix,
    Position, Range, TextEdit,
};

use super::DbtContext;

/// Unused-var hints for `uri` when it is the root dbt_project.yml;
/// `text` is its current content.
pub fn unused_var_diagnostics(ctx: &DbtContext, uri: &str, text: &str) -> Vec<Diagnostic> {
    let (Some(root), Some(reads)) = (ctx.project_root.as_deref(), ctx.var_reads.as_ref()) else {
        return Vec::new();
    };
    let Some(path) = cogitator_lsp::uri_to_fs_path(uri) else {
        return Vec::new();
    };
    if Path::new(&path) != root.join("dbt_project.yml") {
        return Vec::new();
    }
    let own = ctx.project.name.as_str();
    let installed: HashSet<&str> = ctx
        .package_variables
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    let read_by = |project: &str, name: &str| reads.get(project).is_some_and(|r| r.contains(name));
    let read_anywhere = |name: &str| reads.values().any(|r| r.contains(name));
    let mut out = Vec::new();

    for (key, range) in &ctx.project.var_ranges {
        let value = ctx.project.vars.get(key);
        let object = value.is_some_and(|v| v.is_object());
        let block = object
            && (key == own
                || installed.contains(key.as_str())
                || ctx.declared_packages.iter().any(|d| d == key));
        if block || value.is_none() || read_anywhere(key) {
            continue;
        }
        out.push(unused(
            text,
            key,
            *range,
            "no file of the project or its packages reads it",
        ));
    }
    for (block, keys) in &ctx.project.nested_var_ranges {
        let scope_reads: &str = if block == own {
            own
        } else if installed.contains(block.as_str()) {
            block
        } else {
            continue; // an object var, or an uninstalled package's block
        };
        for (key, range) in keys {
            if !read_by(scope_reads, key) {
                let why = if block == own {
                    "no file of the project reads it".to_string()
                } else {
                    format!("package {block} does not read it")
                };
                out.push(unused(text, key, *range, &why));
            }
        }
    }
    out.sort_by_key(|d| (d.range.start.line, d.range.start.character));
    out
}

fn unused(text: &str, name: &str, range: Range, why: &str) -> Diagnostic {
    let mut d = Diagnostic::plain(
        range,
        Guidance::new(format!("var '{name}' is never read"))
            .because(why.to_string())
            .to_diagnostic(),
        diagnostic_severity::HINT,
        "unused-var",
        "cogitator",
    );
    d.tags = Some(vec![diagnostic_tag::UNNECESSARY]);
    d.data = Some(DiagnosticData {
        fix: FixKind::MultiEdit {
            fixes: vec![MultiEditFix {
                title: format!("Remove unused var '{name}'"),
                edits: vec![TextEdit {
                    range: entry_lines(text, range.start.line),
                    new_text: String::new(),
                }],
                preserving: false,
            }],
        },
        token_range: range,
        suggestions: Vec::new(),
        subject: None,
    });
    d
}

/// The whole lines of the YAML entry whose key is on `line`: the key's
/// line and every deeper-indented (or blank) line after it.
pub(crate) fn entry_lines(text: &str, line: u32) -> Range {
    let lines: Vec<&str> = text.lines().collect();
    let indent = |l: &str| l.len() - l.trim_start().len();
    let Some(first) = lines.get(line as usize) else {
        return Range::default();
    };
    let own = indent(first);
    let mut end = line as usize + 1;
    while let Some(l) = lines.get(end) {
        if !l.trim().is_empty() && indent(l) <= own {
            break;
        }
        end += 1;
    }
    // trailing blank lines belong to what follows
    while end > line as usize + 1 && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    Range {
        start: Position { line, character: 0 },
        end: Position {
            line: end as u32,
            character: 0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_entry_spans_its_children() {
        let text = "vars:\n  a: 1\n  b:\n    - x\n    - y\n\n  c: 2\n";
        let cases = [(1, 1, 2), (2, 2, 5), (6, 6, 7)];
        for (line, start, end) in cases {
            let r = entry_lines(text, line);
            assert_eq!((r.start.line, r.end.line), (start, end), "line {line}");
        }
    }
}
