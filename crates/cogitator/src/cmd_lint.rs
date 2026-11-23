//! `cogitator lint`: every diagnostic the server computes without an
//! editor, over every file of the project — for CI, like `dbt lint` or
//! sqlfluff. The per-file set is the server's own
//! (`server::diagnostics::static_diagnostics`: syntax, the cogitator.yml
//! `lint:` checks, dangling refs/sources/docs, YAML validation, macro
//! call checks) plus the project's ast-grep rules.
//!
//! Exit codes: 0 clean, 1 an error (or more warnings than
//! `--max-warnings`), 2 a usage or project error.

use std::path::{Path, PathBuf};

use cogitator_analysis::config_schema::lint_flags;
use cogitator_analysis::features::{self, lint::LintOptions};
use cogitator_analysis::Document;
use cogitator_lsp::{diagnostic_severity, Diagnostic};
use cogitator_parser::{DocumentParser, JinjaSqlParser};

use crate::cmd_common::{self, Project};

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    /// `path:line:col: severity code: message`
    Text,
    /// One JSON array of findings.
    Json,
    /// GitHub Actions workflow annotations.
    Github,
}

#[derive(Debug, clap::Args)]
pub struct LintArgs {
    /// The dbt project (the directory of dbt_project.yml, or below it).
    #[arg(long = "project-dir", default_value = ".")]
    project_dir: PathBuf,
    /// Output format.
    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,
    /// Fail (exit 1) when more than this many warnings are found.
    #[arg(long = "max-warnings")]
    max_warnings: Option<usize>,
    /// Files or directories to lint (default: the whole project).
    paths: Vec<PathBuf>,
}

/// One finding of one file.
struct Finding {
    path: PathBuf,
    diagnostic: Diagnostic,
}

pub fn run(args: &LintArgs, profiles_dir: Option<&str>, target: Option<&str>) -> i32 {
    let project = match cmd_common::load_project(&args.project_dir, profiles_dir, target) {
        Ok(project) => project,
        Err(guidance) => return cmd_common::fail(&guidance),
    };
    let files = match cmd_common::select(lintable_files(&project), &args.paths, lintable) {
        Ok(files) => files,
        Err(guidance) => return cmd_common::fail(&guidance),
    };
    let findings = lint_files(&project, &files);
    match args.format {
        Format::Text => print_text(&findings, files.len()),
        Format::Json => print_json(&findings),
        Format::Github => print_github(&findings),
    }
    exit_code(&findings, args.max_warnings)
}

fn exit_code(findings: &[Finding], max_warnings: Option<usize>) -> i32 {
    let count = |severity| {
        findings
            .iter()
            .filter(|f| f.diagnostic.severity == severity)
            .count()
    };
    let too_many_warnings =
        max_warnings.is_some_and(|max| count(diagnostic_severity::WARNING) > max);
    i32::from(count(diagnostic_severity::ERROR) > 0 || too_many_warnings)
}

fn lintable(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("sql" | "yml" | "yaml")
    )
}

/// The root project's files, its selectors.yml and cogitator.yml.
fn lintable_files(project: &Project) -> Vec<PathBuf> {
    let mut files = features::project_files(&project.ctx);
    for name in ["selectors.yml", "cogitator.yml", ".cogitator.yml"] {
        let path = project.root.join(name);
        if path.is_file() {
            files.push(path);
        }
    }
    files.retain(|f| lintable(f));
    files.sort();
    files.dedup();
    files
}

fn lint_files(project: &Project, files: &[PathBuf]) -> Vec<Finding> {
    let flags = project.config.lint.flags(lint_flags::DEFAULT_ON);
    let lints = LintOptions::from_flags(flags);
    let rules = cogitator_astgrep::Rules::load(&project.root);
    let mut out = Vec::new();
    for path in files {
        let Ok(text) = std::fs::read_to_string(path) else {
            eprintln!(
                "cogitator: skipped {} (unreadable)",
                cmd_common::shown(path)
            );
            continue;
        };
        let uri = cogitator_lsp::fs_path_to_uri(path);
        let Ok(parsed) = JinjaSqlParser.parse(&text, &project.ctx.dialect) else {
            continue;
        };
        let doc = Document {
            uri: uri.clone().into(),
            text: text.into(),
            parsed,
            generation: 0,
        };
        let mut items = crate::server::diagnostics::static_diagnostics(
            &project.ctx,
            Some(&doc),
            &uri,
            lints,
            None,
        );
        items.extend(crate::server::astgrep::diagnostics(&rules, &uri, &doc.text));
        if features::selectors_yml::is_selectors_file(&uri) {
            let open = crate::server::selectors::Open::of(Vec::new());
            items.extend(crate::server::selectors::unmatched(
                &project.ctx,
                open,
                &doc.text,
            ));
        }
        items.sort_by_key(|d| (d.range.start.line, d.range.start.character));
        out.extend(items.into_iter().map(|diagnostic| Finding {
            path: path.clone(),
            diagnostic,
        }));
    }
    out
}

fn severity_name(severity: i32) -> &'static str {
    match severity {
        diagnostic_severity::ERROR => "error",
        diagnostic_severity::WARNING => "warning",
        diagnostic_severity::INFO => "info",
        _ => "hint",
    }
}

fn print_text(findings: &[Finding], files: usize) {
    for f in findings {
        let d = &f.diagnostic;
        let mut lines = d.message.lines();
        println!(
            "{}:{}:{}: {} {}: {}",
            cmd_common::shown(&f.path),
            d.range.start.line + 1,
            d.range.start.character + 1,
            severity_name(d.severity),
            if d.code.is_empty() { "-" } else { &d.code },
            lines.next().unwrap_or("")
        );
        for line in lines {
            println!("    {line}");
        }
    }
    let count = |s| {
        findings
            .iter()
            .filter(|f| f.diagnostic.severity == s)
            .count()
    };
    eprintln!(
        "cogitator lint: {} error(s), {} warning(s), {} other in {files} file(s)",
        count(diagnostic_severity::ERROR),
        count(diagnostic_severity::WARNING),
        findings.len() - count(diagnostic_severity::ERROR) - count(diagnostic_severity::WARNING),
    );
}

fn print_json(findings: &[Finding]) {
    let items: Vec<serde_json::Value> = findings
        .iter()
        .map(|f| {
            let d = &f.diagnostic;
            serde_json::json!({
                "path": cmd_common::shown(&f.path),
                "line": d.range.start.line + 1,
                "column": d.range.start.character + 1,
                "endLine": d.range.end.line + 1,
                "endColumn": d.range.end.character + 1,
                "severity": severity_name(d.severity),
                "code": d.code,
                "source": d.source,
                "message": d.message,
            })
        })
        .collect();
    println!(
        "{}",
        serde_json::to_string_pretty(&items).unwrap_or_else(|_| "[]".to_string())
    );
}

/// `::error file=…,line=…,col=…::message` (GitHub's workflow commands).
fn print_github(findings: &[Finding]) {
    for f in findings {
        let d = &f.diagnostic;
        let level = match d.severity {
            diagnostic_severity::ERROR => "error",
            diagnostic_severity::WARNING => "warning",
            _ => "notice",
        };
        println!(
            "::{level} file={},line={},col={},endLine={},endColumn={},title={}::{}",
            escape_property(&cmd_common::shown(&f.path)),
            d.range.start.line + 1,
            d.range.start.character + 1,
            d.range.end.line + 1,
            d.range.end.character + 1,
            escape_property(if d.code.is_empty() {
                "cogitator"
            } else {
                &d.code
            }),
            escape_data(&d.message),
        );
    }
}

fn escape_data(text: &str) -> String {
    text.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

fn escape_property(text: &str) -> String {
    escape_data(text).replace(':', "%3A").replace(',', "%2C")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_escapes_follow_the_workflow_command_rules() {
        assert_eq!(escape_data("a%b\nc\r"), "a%25b%0Ac%0D");
        assert_eq!(escape_property("a:b,c"), "a%3Ab%2Cc");
    }

    #[test]
    fn exit_code_counts_errors_and_warnings() {
        let finding = |severity| Finding {
            path: PathBuf::from("m.sql"),
            diagnostic: Diagnostic::plain(Default::default(), "x".into(), severity, "c", "s"),
        };
        assert_eq!(exit_code(&[], None), 0);
        assert_eq!(exit_code(&[finding(diagnostic_severity::ERROR)], None), 1);
        let warnings = [
            finding(diagnostic_severity::WARNING),
            finding(diagnostic_severity::WARNING),
        ];
        assert_eq!(exit_code(&warnings, None), 0);
        assert_eq!(exit_code(&warnings, Some(2)), 0);
        assert_eq!(exit_code(&warnings, Some(1)), 1);
    }
}
