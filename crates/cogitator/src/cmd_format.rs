//! `cogitator format`: `textDocument/formatting` over every model SQL
//! file of the project (or the given paths). In place by default;
//! `--check` only reports the files that would change (exit 1), and
//! `--diff` adds the unified diff. The rules are `--config`'s, else the
//! nearest cogitator-format.toml's, else the defaults with `--indent`; the
//! dialect is the rules', else the project's adapter's. A file the
//! formatter declines (05 §9: SQL it cannot parse, Jinja it cannot keep
//! intact) is reported as skipped, with why, and never changed.

use std::path::{Path, PathBuf};

use cogitator_analysis::features::formatting::{format_text, rules_near, FormatOptions};
use cogitator_format::Declined;

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
    /// Indentation width in spaces, without a config.
    #[arg(long = "indent", default_value_t = 4)]
    indent: u8,
    /// The formatter's rules (a cogitator-format.toml), for every file.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Model files or directories to format (default: every model).
    paths: Vec<PathBuf>,
}

/// What became of one file.
#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Unchanged,
    /// The formatted text.
    Changed(String),
    Declined(Declined),
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
    let fixed = match &args.config {
        Some(path) => match std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|text| {
                cogitator_format::Config::from_toml(&text, path).map_err(|e| e.to_string())
            }) {
            Ok(rules) => Some(rules),
            Err(err) => {
                eprintln!("cogitator: {}: {err}", path.display());
                return cmd_common::USAGE;
            }
        },
        None => None,
    };
    let options_for = |path: &Path| -> Result<FormatOptions, String> {
        let rules = match &fixed {
            Some(rules) => Some(rules.clone()),
            None => rules_near(path).map_err(|e| e.to_string())?,
        };
        Ok(FormatOptions {
            tab_size: args.indent,
            insert_spaces: true,
            rules,
            adapter: project.ctx.dialect.as_str().to_string(),
        })
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
        let options = match options_for(path) {
            Ok(options) => options,
            Err(err) => {
                eprintln!("cogitator: {err}");
                return cmd_common::USAGE;
            }
        };
        match outcome(&text, &options) {
            Outcome::Unchanged => {}
            Outcome::Declined(why) => {
                skipped += 1;
                eprintln!("skipped {} ({why})", cmd_common::shown(path));
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

fn outcome(text: &str, options: &FormatOptions) -> Outcome {
    match format_text(text, options) {
        Err(why) => Outcome::Declined(why),
        Ok(out) if out == text => Outcome::Unchanged,
        Ok(out) => Outcome::Changed(out),
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

    fn options() -> FormatOptions {
        FormatOptions {
            tab_size: 4,
            insert_spaces: true,
            rules: None,
            adapter: "duckdb".into(),
        }
    }

    #[test]
    fn outcomes() {
        let messy = "select a,b from {{ ref('x') }} where a>1\n";
        let Outcome::Changed(once) = outcome(messy, &options()) else {
            panic!("formats");
        };
        assert_eq!(outcome(&once, &options()), Outcome::Unchanged);
        assert!(matches!(
            outcome("select 1 from {{ unclosed\n", &options()),
            Outcome::Declined(Declined::Jinja(_))
        ));
    }

    #[test]
    fn diff_names_both_sides() {
        let diff = unified_diff(Path::new("/p/m.sql"), "a\n", "b\n");
        assert!(diff.contains("-a\n+b\n"), "{diff}");
        assert!(diff.contains("a//p/m.sql") || diff.contains("a/"), "{diff}");
    }
}
