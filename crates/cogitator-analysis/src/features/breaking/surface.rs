//! The editor surfaces of a breaking-change report: one diagnostic per
//! removed or retyped column, the summary line and the code lens.

use cogitator_lsp::{
    diagnostic_severity, CodeLens, Command, Diagnostic, DiagnosticData,
    DiagnosticRelatedInformation, FixKind, Location, MultiEditFix, Range,
};

use super::downstream::Severity;
use super::{BaselineSource, BreakingReport, ChangeKind, ColumnImpact, Status};

pub const BREAKING_CODE: &str = "breaking-change";

pub(super) fn summary(report: &BreakingReport) -> String {
    let n = report.breaking();
    if n == 0 {
        return format!(
            "no breaking changes against {}",
            report
                .baseline
                .as_ref()
                .map_or("the built schema", |b| b.source.file())
        );
    }
    let parts: Vec<String> = report.impacts.iter().map(describe_change).collect();
    format!("{n} breaking change(s): {}", parts.join(", "))
}

fn describe_change(impact: &ColumnImpact) -> String {
    match impact.change {
        ChangeKind::Removed => format!("{} removed", impact.column),
        ChangeKind::Retyped => format!("{} retyped", impact.column),
    }
}

/// The diagnostics of a compared report (none for any other status):
/// what the open buffer's edit breaks against the saved file first,
/// then what the model breaks against the last build, a column flagged
/// by the edit once. Each sits at its column's place in the SQL
/// (`anchor.rs`), the top of the file when there is none.
pub fn diagnostics(report: &BreakingReport) -> Vec<Diagnostic> {
    let mut out: Vec<Diagnostic> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    let edit = report.since_save.as_deref();
    for r in edit.into_iter().chain(std::iter::once(report)) {
        if r.status != Status::Compared {
            continue;
        }
        let file = r.baseline.as_ref().map_or("target/", |b| b.source.file());
        for impact in &r.impacts {
            if seen.iter().any(|c| c.eq_ignore_ascii_case(&impact.column)) {
                continue;
            }
            seen.push(impact.column.clone());
            out.push(diagnostic(r, impact, file));
        }
    }
    out
}

fn diagnostic(report: &BreakingReport, impact: &ColumnImpact, file: &str) -> Diagnostic {
    let severity = match impact.severity {
        Severity::Error => diagnostic_severity::ERROR,
        Severity::Warning => diagnostic_severity::WARNING,
        Severity::Information => diagnostic_severity::INFO,
    };
    let mut d = Diagnostic::plain(
        impact.range.unwrap_or_default(),
        message(report, impact, file),
        severity,
        BREAKING_CODE,
        "cogitator",
    );
    if !impact.related.is_empty() {
        d.related_information = Some(
            impact
                .related
                .iter()
                .map(|r| DiagnosticRelatedInformation {
                    location: Location {
                        uri: r.uri.clone(),
                        range: r.range,
                    },
                    message: r.message.clone(),
                })
                .collect(),
        );
    }
    if let Some(edit) = &impact.keep_alias {
        d.data = Some(DiagnosticData {
            fix: FixKind::MultiEdit {
                fixes: vec![MultiEditFix {
                    title: format!("Keep '{}' as an alias", impact.column),
                    edits: vec![edit.clone()],
                    preserving: false,
                }],
            },
            token_range: edit.range,
            suggestions: Vec::new(),
            subject: None,
        });
    }
    d
}

fn plural(n: usize, one: &str) -> String {
    match n {
        1 => format!("1 {one}"),
        n => format!("{n} {one}s"),
    }
}

/// "breaks 3 models, 2 tests, 1 exposure": what relies on the column,
/// counted (the related information lists each).
pub(super) fn breaks(report: &BreakingReport, impact: &ColumnImpact) -> String {
    let mut models: Vec<&str> = impact.downstream.iter().map(|d| d.model.as_str()).collect();
    models.sort_unstable();
    models.dedup();
    let tests = impact.tests.len()
        + impact
            .downstream
            .iter()
            .map(|d| d.tests.len())
            .sum::<usize>();
    let mut semantic: Vec<&str> = impact
        .semantic
        .iter()
        .map(|s| s.semantic_model.as_str())
        .collect();
    semantic.sort_unstable();
    semantic.dedup();
    let parts: Vec<String> = [
        (models.len(), "model"),
        (tests, "test"),
        (semantic.len(), "semantic model"),
        (impact.metrics.len(), "metric"),
        (report.exposures.len(), "exposure"),
    ]
    .into_iter()
    .filter(|(n, _)| *n > 0)
    .map(|(n, what)| plural(n, what))
    .collect();
    match parts.is_empty() {
        true => "nothing downstream reads it".to_string(),
        false => format!("breaks {}", parts.join(", ")),
    }
}

fn message(report: &BreakingReport, impact: &ColumnImpact, file: &str) -> String {
    let mut text = match impact.change {
        ChangeKind::Removed => {
            let renamed = report
                .diff
                .removed
                .iter()
                .find(|r| r.name == impact.column)
                .and_then(|r| r.possibly_renamed_to.as_deref())
                .map(|to| format!(" (possibly renamed to '{to}')"))
                .unwrap_or_default();
            format!("column '{}' was removed{renamed}", impact.column)
        }
        ChangeKind::Retyped => {
            let change = report.diff.retyped.iter().find(|t| t.name == impact.column);
            match change {
                Some(t) => format!("column '{}' changed type {} → {}", t.name, t.from, t.to),
                None => format!("column '{}' changed type", impact.column),
            }
        }
    };
    let since = match report.baseline.as_ref().map(|b| b.source) {
        Some(BaselineSource::Saved) => " in this edit (against the saved file)".to_string(),
        _ => format!(" since the last build ({file})"),
    };
    text.push_str(&since);
    text.push_str(": ");
    text.push_str(&breaks(report, impact));
    if report.public {
        text.push_str("; the model is public: consumers outside the project may read it");
    }
    text
}

/// "N breaking changes", running `dbt.breakingChanges`; none at zero.
pub fn lens(report: &BreakingReport, uri: &str) -> Option<CodeLens> {
    let mut columns: Vec<String> = Vec::new();
    let edit = report.since_save.as_deref();
    for r in edit.into_iter().chain(std::iter::once(report)) {
        if r.status != Status::Compared {
            continue;
        }
        let names = r.diff.removed.iter().map(|c| &c.name);
        for name in names.chain(r.diff.retyped.iter().map(|c| &c.name)) {
            if !columns.iter().any(|c| c.eq_ignore_ascii_case(name)) {
                columns.push(name.clone());
            }
        }
    }
    let n = columns.len();
    if n == 0 {
        return None;
    }
    Some(CodeLens {
        range: Range::default(),
        command: Command {
            title: match n {
                1 => "1 breaking change".to_string(),
                n => format!("{n} breaking changes"),
            },
            command: "dbt.breakingChanges".to_string(),
            arguments: vec![serde_json::json!({ "uri": uri })],
        },
    })
}
