//! Macro definitions across own project + packages (03 §5, 06 §2.2),
//! served from the scan cache: unchanged files are not re-read.

use std::collections::HashMap;
use std::path::Path;

use cogitator_lsp::{Position, Range};
use cogitator_parser::jinja_scan::{self, RawMacro};

use crate::cache::FileCache;
use crate::util;

use super::{MacroDetails, ProjectDetails};

pub(super) fn build_macro_details(
    process_list: &[ProjectDetails],
    cache: &mut FileCache<Vec<RawMacro>>,
) -> HashMap<String, HashMap<String, MacroDetails>> {
    let mut map: HashMap<String, HashMap<String, MacroDetails>> = HashMap::new();
    for p in process_list {
        let per_project = map.entry(p.file.name.clone()).or_default();
        for rel in &p.file.macro_paths {
            let dir = p.root.join(rel);
            if std::fs::read_dir(&dir).is_err() {
                continue;
            }
            for file in util::walk_filepath(&dir, ".sql") {
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
    map
}

fn details(raw: &RawMacro, project_name: &str, path: &Path) -> MacroDetails {
    MacroDetails {
        name: raw.name.clone(),
        project_name: project_name.to_string(),
        description: raw.signature.clone(),
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
