//! The editor surfaces of a breaking-change report: one diagnostic per
//! removed or retyped column, the summary line and the code lens.

use cogitator_lsp::{diagnostic_severity, CodeLens, Command, Diagnostic, Range};

use super::downstream::Severity;
use super::{BreakingReport, ChangeKind, ColumnImpact, Status};

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

/// The diagnostics of a compared report (none for any other status).
pub fn diagnostics(report: &BreakingReport) -> Vec<Diagnostic> {
    if report.status != Status::Compared {
        return Vec::new();
    }
    let file = report
        .baseline
        .as_ref()
        .map_or("target/", |b| b.source.file());
    report
        .impacts
        .iter()
        .map(|impact| {
            let severity = match impact.severity {
                Severity::Error => diagnostic_severity::ERROR,
                Severity::Warning => diagnostic_severity::WARNING,
                Severity::Information => diagnostic_severity::INFO,
            };
            Diagnostic::plain(
                Range::default(),
                message(report, impact, file),
                severity,
                BREAKING_CODE,
                "cogitator",
            )
        })
        .collect()
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
    text.push_str(&format!(" since the last build ({file})"));
    if !impact.tests.is_empty() {
        text.push_str(&format!("; tested: {}", impact.tests.join(", ")));
    }
    if !impact.downstream.is_empty() {
        let reads: Vec<String> = impact
            .downstream
            .iter()
            .map(|d| {
                let mut tags = Vec::new();
                if d.contracted {
                    tags.push("contracted".to_string());
                }
                if d.public {
                    tags.push("public".to_string());
                }
                if !d.tests.is_empty() {
                    tags.push(format!("tested: {}", d.tests.join(", ")));
                }
                match tags.is_empty() {
                    true => format!("{}.{}", d.model, d.column),
                    false => format!("{}.{} ({})", d.model, d.column, tags.join("; ")),
                }
            })
            .collect();
        text.push_str(&format!("; read by {}", reads.join(", ")));
    }
    if report.public {
        text.push_str("; the model is public: consumers outside the project may read it");
    }
    if !report.exposures.is_empty() {
        text.push_str(&format!("; exposures: {}", report.exposures.join(", ")));
    }
    text
}

/// "N breaking changes", running `dbt.breakingChanges`; none at zero.
pub fn lens(report: &BreakingReport, uri: &str) -> Option<CodeLens> {
    let n = report.breaking();
    if report.status != Status::Compared || n == 0 {
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
