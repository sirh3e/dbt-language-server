//! Macro definitions across own project + packages (03 §5, 06 §2.2),
//! served from the scan cache: unchanged files are not re-read.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::sync::LazyLock;

use cogitator_lsp::{Position, Range};
use cogitator_parser::jinja_scan::{self, RawMacro};

use crate::cache::FileCache;
use crate::util;

use super::{MacroDetails, ProjectDetails};

static MATERIALIZATION_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"\{%-?\s*materialization\s+([A-Za-z_][A-Za-z0-9_]*)").expect("valid regex")
});

/// The names of the `{% materialization name, ... %}` blocks of a macro
/// file, in file order; commented-out ones are left out. dbt keeps them
/// apart from the macros a model may call, so they are scanned apart.
pub(crate) fn materializations_in(text: &str) -> Vec<String> {
    use cogitator_parser::jinja_spans::{blanked, NOT_CODE};
    let live = blanked(text, NOT_CODE);
    MATERIALIZATION_RE
        .captures_iter(&live)
        .map(|c| c[1].to_string())
        .collect()
}

/// Every macro of the project and its packages, and the names of their
/// custom materializations.
pub(super) fn build_macro_details(
    process_list: &[ProjectDetails],
    cache: &mut FileCache<Vec<RawMacro>>,
    materialization_cache: &mut FileCache<Vec<String>>,
) -> (
    HashMap<String, HashMap<String, MacroDetails>>,
    BTreeSet<String>,
) {
    let mut map: HashMap<String, HashMap<String, MacroDetails>> = HashMap::new();
    let mut materializations = BTreeSet::new();
    for p in process_list {
        let per_project = map.entry(p.file.name.clone()).or_default();
        for rel in &p.file.macro_paths {
            let dir = p.root.join(rel);
            if std::fs::read_dir(&dir).is_err() {
                continue;
            }
            for file in util::walk_filepath(&dir, ".sql") {
                if let Some(names) = materialization_cache
                    .get_or_parse(&file, |text, _| Some(materializations_in(text)))
                {
                    materializations.extend(names.iter().cloned());
                }
                let Some(raw_macros) =
                    cache.get_or_parse(&file, |text, _| Some(jinja_scan::extract_macros(text)))
                else {
                    continue;
                };
                per_project.extend(
                    raw_macros
                        .iter()
                        .map(|raw| (raw.name.clone(), details(raw, &p.file.name, &file))),
                );
            }
        }
    }
    cache.sweep();
    materialization_cache.sweep();
    (map, materializations)
}

fn details(raw: &RawMacro, project_name: &str, path: &Path) -> MacroDetails {
    MacroDetails {
        name: raw.name.clone(),
        project_name: project_name.to_string(),
        description: raw.signature.clone(),
        params: raw.params.clone(),
        body: raw.body.clone(),
        uri: path.to_path_buf(),
        range: Range {
            start: Position {
                line: raw.start.0,
                character: raw.start.1,
            },
            end: Position {
                line: raw.end.0,
                character: raw.end.1,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::materializations_in;

    #[test]
    fn materialization_blocks_are_named() {
        let text = "{% materialization lake_table, adapter='duckdb' %}x{% endmaterialization %}\n\
                    {#- {% materialization commented, default %} -#}\n\
                    {%- materialization other, default -%}y{% endmaterialization %}\n\
                    {% macro not_one() %}{% endmacro %}";
        assert_eq!(materializations_in(text), vec!["lake_table", "other"]);
    }
}
