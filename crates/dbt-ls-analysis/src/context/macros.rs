//! Macro definitions across own project + packages (03 §5, 06 §2.2).

use std::collections::HashMap;

use dbt_ls_lsp::{Position, Range};
use dbt_ls_parser::jinja_scan;

use crate::fs_scan::{read_sources, SourceFile};

use super::{MacroDetails, ProjectDetails};

/// Macros across own project + packages (03 §5 / 06 §2.2).
pub(super) fn build_macro_details(
    process_list: &[ProjectDetails],
) -> HashMap<String, HashMap<String, MacroDetails>> {
    process_list
        .iter()
        .map(|p| {
            let files = read_sources(&p.root, &p.file.macro_paths, ".sql");
            (
                p.file.name.clone(),
                macros_from_sources(&p.file.name, &files),
            )
        })
        .fold(HashMap::new(), |mut map, (project, macros)| {
            map.entry(project).or_default().extend(macros);
            map
        })
}

/// Pure: macro definitions across one project's macro files (06 §2.2).
fn macros_from_sources(project_name: &str, files: &[SourceFile]) -> HashMap<String, MacroDetails> {
    files
        .iter()
        .flat_map(|f| {
            jinja_scan::extract_macros(&f.text)
                .into_iter()
                .map(move |raw| (f.path.clone(), raw))
        })
        .map(|(path, raw)| {
            (
                raw.name.clone(),
                MacroDetails {
                    name: raw.name,
                    project_name: project_name.to_string(),
                    description: raw.signature,
                    uri: path,
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
                },
            )
        })
        .collect()
}
