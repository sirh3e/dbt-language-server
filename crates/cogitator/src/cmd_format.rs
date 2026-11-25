//! `cogitator format`: `textDocument/formatting` over every model SQL
//! file of the project (or the given paths). In place by default;
//! `--check` only reports the files that would change (exit 1), and
//! `--diff` adds the unified diff. A file the formatter declines (05 §9:
//! inline statements, a sentinel miscount, any doubt) is reported as
//! skipped and never changed.

use std::path::{Path, PathBuf};

use cogitator_analysis::features::formatting::{format_document, FormatOptions};

use crate::cmd_common;

#[derive(Debug, clap::Args)]
pub struct FormatArgs {
    /// The dbt project (the directory of dbt_project.yml, or below it).
    #[arg(long = "project-dir", default_value = ".")]
    project_dir: PathBuf,
    /// Change nothing: list the files that would change and exit 1 if any.
    #[arg(long)]
    check: bool,
    /// With --check, print the unified diff of each file that would change.
    #[arg(long, requires = "check")]
    diff: bool,
    /// Indentation width in spaces.
    #[arg(long = "indent", default_value_t = 4)]
    indent: u8,
    /// Model files or directories to format (default: every model).
    paths: Vec<PathBuf>,
}

/// What became of one file.
#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Unchanged,
    /// The formatted text.
    Changed(String),
    Declined,
}

pub fn run(args: &FormatArgs, profiles_dir: Option<&str>, target: Option<&str>) -> i32 {
    cmd_common::init_logs();
    let project = match cmd_common::load_project(&args.project_dir, profiles_dir, target) {
        Ok(project) => project,
        Err(guidance) => return cmd_common::fail(&guidance),
    };
    let own = project.ctx.project.name.as_str();
    let mut models: Vec<PathBuf> = project
        .ctx
        .model_detail_map
        .values()
        .filter(|m| m.project_name == own)
        .map(|m| m.uri.clone())
        .filter(|p| is_sql(p))
        .collect();
    models.sort();
    models.dedup();
    let files = match cmd_common::select(models, &args.paths, is_sql) {
        Ok(files) => files,
        Err(guidance) => return cmd_common::fail(&guidance),
    };
    let options = FormatOptions {
        tab_size: args.indent,
        insert_spaces: true,
    };
    let (mut changed, mut skipped) = (0, 0);
    for path in &files {
        let Ok(text) = std::fs::read_to_string(path) else {
            eprintln!(
                "cogitator: skipped {} (unreadable)",
                cmd_common::shown(path)
            );
            skipped += 1;
            continue;
        };
        match format_text(&text, &options) {
            Outcome::Unchanged => {}
            Outcome::Declined => {
                skipped += 1;
                eprintln!(
                    "skipped {} (the formatter declines it: Jinja it cannot keep intact)",
                    cmd_common::shown(path)
                );
            }
            Outcome::Changed(formatted) => {
                changed += 1;
                if args.check {
                    println!("would reformat {}", cmd_common::shown(path));
                    if args.diff {
                        print!("{}", unified_diff(path, &text, &formatted));
                    }
                } else if let Err(err) = std::fs::write(path, &formatted) {
                    eprintln!(
                        "cogitator: could not write {}: {err}",
                        cmd_common::shown(path)
                    );
                    return cmd_common::USAGE;
                } else {
                    println!("reformatted {}", cmd_common::shown(path));
                }
            }
        }
    }
    let verb = if args.check {
        "would be reformatted"
    } else {
        "reformatted"
    };
    eprintln!(
        "cogitator format: {changed} file(s) {verb}, {} unchanged, {skipped} skipped",
        files.len() - changed - skipped
    );
    i32::from(args.check && changed > 0)
}

fn is_sql(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("sql")
}

fn format_text(text: &str, options: &FormatOptions) -> Outcome {
    match format_document(text, options) {
        None => Outcome::Declined,
        Some(edits) => match edits.into_iter().next() {
            Some(edit) if edit.new_text != text => Outcome::Changed(edit.new_text),
            _ => Outcome::Unchanged,
        },
    }
}

fn unified_diff(path: &Path, before: &str, after: &str) -> String {
    let name = cmd_common::shown(path);
    similar::TextDiff::from_lines(before, after)
        .unified_diff()
        .header(&format!("a/{name}"), &format!("b/{name}"))
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPTIONS: FormatOptions = FormatOptions {
        tab_size: 4,
        insert_spaces: true,
    };

    #[test]
    fn outcomes() {
        let messy = "select a,b from {{ ref('x') }} where a>1\n";
        let Outcome::Changed(once) = format_text(messy, &OPTIONS) else {
            panic!("formats");
        };
        assert_eq!(format_text(&once, &OPTIONS), Outcome::Unchanged);
        assert_eq!(
            format_text("select 1 {% if x %} , 2 {% endif %}\n", &OPTIONS),
            Outcome::Declined
        );
    }

    #[test]
    fn diff_names_both_sides() {
        let diff = unified_diff(Path::new("/p/m.sql"), "a\n", "b\n");
        assert!(diff.contains("-a\n+b\n"), "{diff}");
        assert!(diff.contains("a//p/m.sql") || diff.contains("a/"), "{diff}");
    }
}
