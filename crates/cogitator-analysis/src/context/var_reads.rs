//! Which var names each project reads (unused-var diagnostics): every
//! `var('…')` in the SQL and YAML of a project's resource paths and in
//! its dbt_project.yml, by project name. Per-file results are cached
//! across refreshes like the other scans.

use std::collections::{HashMap, HashSet};

use cogitator_dialects::Dialect;
use cogitator_parser::{DocumentParser, JinjaSqlParser, TokenKind};

use super::ProjectDetails;
use crate::cache::FileCache;

/// Project name → the var names its files read.
pub type VarReads = HashMap<String, HashSet<String>>;

/// The var names a text reads (pure).
pub fn var_names_read(text: &str, dialect: &Dialect) -> Vec<String> {
    if !text.contains("var") {
        return Vec::new();
    }
    let Ok(parsed) = JinjaSqlParser.parse(text, dialect) else {
        return Vec::new();
    };
    let mut names: Vec<String> = parsed
        .tokens
        .iter()
        .filter(|t| t.kind == TokenKind::Var)
        .map(|t| parsed.literal(t).to_string())
        .collect();
    names.sort();
    names.dedup();
    names
}

pub(super) fn build_var_reads(
    projects: &[ProjectDetails],
    dialect: &Dialect,
    cache: &mut FileCache<Vec<String>>,
) -> VarReads {
    let mut out = VarReads::new();
    for p in projects {
        let file = &p.file;
        let dirs = file
            .model_paths
            .iter()
            .chain(&file.macro_paths)
            .chain(&file.test_paths)
            .chain(&file.snapshot_paths)
            .chain(&file.analysis_paths)
            .chain(&file.seed_paths);
        let mut files = vec![p.root.join("dbt_project.yml")];
        for dir in dirs {
            let dir = p.root.join(dir);
            for ext in [".sql", ".yml", ".yaml"] {
                files.extend(crate::util::walk_filepath(&dir, ext));
            }
        }
        let names = out.entry(file.name.clone()).or_default();
        for path in files {
            if let Some(read) =
                cache.get_or_parse(&path, |text, _| Some(var_names_read(text, dialect)))
            {
                names.extend(read.iter().cloned());
            }
        }
    }
    cache.sweep();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_read_by_a_text() {
        let cases = [
            ("select {{ var('a') }}, {{ var(\"b\", 1) }}", vec!["a", "b"]),
            ("x: \"{{ var('c') }}\"", vec!["c"]),
            ("select 1", vec![]),
            ("{{ env_var('E') }}", vec![]),
        ];
        for (text, want) in cases {
            assert_eq!(var_names_read(text, &Dialect::default()), want, "{text}");
        }
    }
}
