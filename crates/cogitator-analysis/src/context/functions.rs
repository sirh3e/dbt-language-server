//! dbt user-defined functions (dbt 1.11+): what `{{ function('name') }}`
//! can name. A function is a body file under the function paths
//! (`functions/is_positive_int.sql`, `.py`, `.js`) plus its `functions:`
//! entry in a properties file there; either alone still counts, so a
//! function half written still gets hover and definition.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use cogitator_lsp::{Position, Range};
use cogitator_schemas::{parse_functions, FunctionEntry};

use super::ProjectDetails;
use crate::cache::FileCache;
use crate::util;

pub(super) type FunctionsCache = FileCache<Vec<FunctionEntry>>;

/// The body languages dbt takes, by file extension.
const BODY_LANGUAGES: &[(&str, &str)] =
    &[(".sql", "sql"), (".py", "python"), (".js", "javascript")];

/// One user-defined function of a project.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FunctionDetails {
    pub name: String,
    pub project_name: String,
    /// The body file, when there is one.
    pub body: Option<PathBuf>,
    /// `sql`, `python` or `javascript`, from the body's extension.
    pub language: Option<&'static str>,
    /// The properties file with the `functions:` entry, and the entry.
    pub yaml: Option<PathBuf>,
    pub entry: Option<FunctionEntry>,
}

impl FunctionDetails {
    /// Where go-to-definition lands: the body, else the YAML entry.
    pub fn definition(&self) -> Option<(PathBuf, Range)> {
        match (&self.body, &self.yaml, &self.entry) {
            (Some(body), _, _) => Some((body.clone(), Range::zero_width(Position::default()))),
            (None, Some(yaml), Some(entry)) => {
                Some((yaml.clone(), Range::zero_width(entry.name_pos)))
            }
            _ => None,
        }
    }

    /// The signature as SQL reads it: `is_positive_int(a_string string) -> boolean`.
    pub fn signature(&self) -> String {
        let Some(entry) = &self.entry else {
            return format!("{}(…)", self.name);
        };
        let args: Vec<String> = entry
            .arguments
            .iter()
            .map(|a| {
                let typed = [a.name.as_str(), a.data_type.as_str()]
                    .iter()
                    .filter(|s| !s.is_empty())
                    .copied()
                    .collect::<Vec<_>>()
                    .join(" ");
                match &a.default_value {
                    Some(d) => format!("{typed} = {d}"),
                    None => typed,
                }
            })
            .collect();
        let returns = entry
            .returns
            .as_ref()
            .filter(|r| !r.data_type.is_empty())
            .map(|r| format!(" -> {}", r.data_type))
            .unwrap_or_default();
        format!("{}({}){returns}", self.name, args.join(", "))
    }

    /// Hover and completion documentation, as markdown.
    pub fn markdown(&self) -> String {
        let fence = self.language.unwrap_or("sql");
        let mut out = format!("```{fence}\n{}\n```", self.signature());
        let Some(entry) = &self.entry else {
            out.push_str("\n\nNo `functions:` entry documents this function.");
            return out;
        };
        if !entry.description.is_empty() {
            out.push_str(&format!("\n\n{}", entry.description));
        }
        let documented: Vec<String> = entry
            .arguments
            .iter()
            .filter(|a| !a.description.is_empty())
            .map(|a| format!("- `{}`: {}", a.name, a.description))
            .collect();
        if !documented.is_empty() {
            out.push_str(&format!("\n\n**Arguments**\n\n{}", documented.join("\n")));
        }
        if let Some(r) = entry.returns.as_ref().filter(|r| !r.description.is_empty()) {
            out.push_str(&format!("\n\n**Returns** {}", r.description));
        }
        let facts: Vec<String> = [
            entry.function_type.as_deref().map(|t| format!("type: {t}")),
            entry
                .volatility
                .as_deref()
                .map(|v| format!("volatility: {v}")),
            self.language.map(|l| format!("language: {l}")),
            (!entry.overloads.is_empty()).then(|| format!("overloads: {}", entry.overloads.len())),
            Some(format!("project: {}", self.project_name)),
        ]
        .into_iter()
        .flatten()
        .collect();
        out.push_str(&format!("\n\n_{}_", facts.join(" · ")));
        out
    }
}

/// What the function paths of every project hold.
#[derive(Debug, Default)]
pub(super) struct Functions {
    /// By name, own project first: on a name both define, the first
    /// project scanned wins, as for docs blocks.
    pub(super) details: HashMap<String, FunctionDetails>,
    /// Every body file, overloads' and shadowed packages' included.
    pub(super) bodies: HashSet<PathBuf>,
}

pub(super) fn build_function_details(
    projects: &[ProjectDetails],
    cache: &mut FunctionsCache,
) -> Functions {
    let mut out = Functions::default();
    for project in projects {
        let dirs = function_dirs(&project.root, project);
        out.bodies.extend(
            dirs.iter()
                .flat_map(|dir| body_files(dir))
                .map(|(path, _, _)| path),
        );
        for (name, details) in project_functions(&dirs, project, cache) {
            out.details.entry(name).or_insert(details);
        }
    }
    cache.sweep();
    out
}

fn function_dirs(root: &Path, project: &ProjectDetails) -> Vec<PathBuf> {
    project
        .file
        .function_paths
        .iter()
        .map(|rel| root.join(rel))
        .filter(|dir| dir.is_dir())
        .collect()
}

fn project_functions(
    dirs: &[PathBuf],
    project: &ProjectDetails,
    cache: &mut FunctionsCache,
) -> HashMap<String, FunctionDetails> {
    let entries: Vec<(PathBuf, FunctionEntry)> = dirs
        .iter()
        .flat_map(|dir| yaml_files(dir))
        .flat_map(|file| {
            let parsed = cache.get_or_parse(&file, |text, path| parse_functions(text, path).ok());
            parsed
                .map(|entries| entries.iter().cloned().collect::<Vec<_>>())
                .unwrap_or_default()
                .into_iter()
                .map(move |e| (file.clone(), e))
        })
        .collect();
    let overload_bodies: HashSet<&str> = entries
        .iter()
        .flat_map(|(_, e)| e.overloads.iter().map(String::as_str))
        .collect();

    let mut out: HashMap<String, FunctionDetails> = HashMap::new();
    for (path, language, name) in dirs.iter().flat_map(|dir| body_files(dir)) {
        if overload_bodies.contains(name.as_str()) {
            continue;
        }
        out.entry(name.clone()).or_insert(FunctionDetails {
            name,
            project_name: project.file.name.clone(),
            body: Some(path),
            language: Some(language),
            ..Default::default()
        });
    }
    for (path, entry) in entries {
        let details = out.entry(entry.name.clone()).or_insert(FunctionDetails {
            name: entry.name.clone(),
            project_name: project.file.name.clone(),
            ..Default::default()
        });
        if details.entry.is_none() {
            details.yaml = Some(path);
            details.entry = Some(entry);
        }
    }
    out
}

fn yaml_files(dir: &Path) -> Vec<PathBuf> {
    [".yml", ".yaml"]
        .iter()
        .flat_map(|ext| util::walk_filepath(dir, ext))
        .collect()
}

/// `(path, language, function name)` of every body file under `dir`.
fn body_files(dir: &Path) -> Vec<(PathBuf, &'static str, String)> {
    BODY_LANGUAGES
        .iter()
        .flat_map(|(ext, language)| {
            util::walk_filepath(dir, ext)
                .into_iter()
                .filter_map(move |path| {
                    let stem = path.file_stem()?.to_string_lossy().into_owned();
                    Some((path, *language, stem))
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cogitator_schemas::{FunctionArgument, FunctionReturns};
    use pretty_assertions::assert_eq;

    fn details(entry: Option<FunctionEntry>) -> FunctionDetails {
        FunctionDetails {
            name: "is_positive_int".into(),
            project_name: "shop".into(),
            body: Some(PathBuf::from("/p/functions/is_positive_int.sql")),
            language: Some("sql"),
            yaml: entry.as_ref().map(|_| PathBuf::from("/p/functions/f.yml")),
            entry,
        }
    }

    fn entry() -> FunctionEntry {
        FunctionEntry {
            name: "is_positive_int".into(),
            description: "Whether a string is a positive integer".into(),
            arguments: vec![
                FunctionArgument {
                    name: "a_string".into(),
                    data_type: "string".into(),
                    description: "The text".into(),
                    default_value: None,
                },
                FunctionArgument {
                    name: "strict".into(),
                    data_type: "boolean".into(),
                    default_value: Some("true".into()),
                    ..Default::default()
                },
            ],
            returns: Some(FunctionReturns {
                data_type: "boolean".into(),
                description: String::new(),
            }),
            function_type: Some("scalar".into()),
            ..Default::default()
        }
    }

    #[test]
    fn the_signature_reads_like_sql() {
        assert_eq!(
            details(Some(entry())).signature(),
            "is_positive_int(a_string string, strict boolean = true) -> boolean"
        );
        assert_eq!(details(None).signature(), "is_positive_int(…)");
    }

    #[test]
    fn the_markdown_says_what_the_entry_says() {
        assert_eq!(
            details(Some(entry())).markdown(),
            "```sql\nis_positive_int(a_string string, strict boolean = true) -> boolean\n```\
             \n\nWhether a string is a positive integer\
             \n\n**Arguments**\n\n- `a_string`: The text\
             \n\n_type: scalar · language: sql · project: shop_"
        );
    }

    #[test]
    fn definition_prefers_the_body() {
        let (path, _) = details(Some(entry())).definition().unwrap();
        assert_eq!(path, PathBuf::from("/p/functions/is_positive_int.sql"));
        let yaml_only = FunctionDetails {
            body: None,
            language: None,
            ..details(Some(FunctionEntry {
                name_pos: Position {
                    line: 3,
                    character: 10,
                },
                ..entry()
            }))
        };
        assert_eq!(
            yaml_only.definition(),
            Some((
                PathBuf::from("/p/functions/f.yml"),
                Range::zero_width(Position {
                    line: 3,
                    character: 10
                })
            ))
        );
    }
}
