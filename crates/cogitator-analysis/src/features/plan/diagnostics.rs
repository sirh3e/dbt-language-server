//! Plan findings as LSP diagnostics (specs/ir-pipeline/08). Pure over a
//! plan, its text, its suppressions and the settings; the server shell
//! decides whether a plan is available at all.
//!
//! Spans are byte offsets; [`span_range`] is the one place they become
//! LSP positions (UTF-16 columns).

use cogitator_ir::Span;
use cogitator_lsp::{
    diagnostic_severity, Diagnostic, DiagnosticData, FixKind, MultiEditFix, Position, Range,
    TextEdit,
};
use cogitator_parser::LineTable;
use cogitator_rules::meta::Severity;

use super::diagnostics_config::PerfSettings;
use super::report::{EvidenceView, FindingView, FixView, QueryPlan};
use super::suppress::Suppressions;

/// May this finding become a diagnostic at all? The eligibility rule:
/// its spans must map onto the document and be fresh, which each finding
/// records.
pub(super) fn eligible(finding: &FindingView) -> bool {
    finding.diagnostics_eligible
}

/// Maps one plan to its diagnostics: findings, then unknown-rule warnings.
pub fn plan_diagnostics(
    text: &str,
    plan: &QueryPlan,
    suppressions: &Suppressions,
    settings: &PerfSettings,
) -> Vec<Diagnostic> {
    if !settings.active() {
        return Vec::new();
    }
    let table = LineTable::new(text);
    let mut out = finding_diagnostics(text, &table, &plan.findings, settings);
    out.extend(suppressions.unknown.iter().filter_map(|unknown| {
        let range = span_range(text, &table, unknown.span)?;
        Some(Diagnostic::plain(
            range,
            unknown.message.clone(),
            diagnostic_severity::WARNING,
            "cogitator-suppress",
            "cogitator",
        ))
    }));
    out
}

/// The diagnostics of findings that are not suppressed, at the severity
/// the settings give them.
pub(super) fn finding_diagnostics(
    text: &str,
    table: &LineTable,
    findings: &[FindingView],
    settings: &PerfSettings,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for finding in findings.iter().filter(|f| !f.suppressed) {
        if !eligible(finding) {
            continue;
        }
        let Some(severity) = settings.severity(finding.id, parse_severity(finding.severity)) else {
            continue;
        };
        let Some(range) = span_range(text, table, finding.span) else {
            tracing::debug!(
                rule = finding.id,
                "finding span is outside the text; dropped"
            );
            continue;
        };
        let code = super::diagnostics_config::diagnostic_code(finding.id);
        let mut diagnostic = Diagnostic::plain(
            range,
            message(finding),
            to_lsp(severity),
            code.into_owned(),
            "cogitator",
        );
        diagnostic.data = fix_payload(text, table, finding, range);
        if let Some(subject) = &finding.subject {
            // the YAML fix is the code action's to build, from the file
            let data = diagnostic.data.get_or_insert_with(|| DiagnosticData {
                fix: FixKind::MultiEdit { fixes: Vec::new() },
                token_range: range,
                suggestions: Vec::new(),
                subject: None,
            });
            data.subject = Some(subject.clone());
        }
        out.push(diagnostic);
    }
    out
}

/// The schema job's correctness checks of `model` (`schemas::checks`) as
/// diagnostics, when they were computed from exactly `text` and the plan
/// is not needed anyway (then [`plan_diagnostics`] reports them).
pub fn snapshot_diagnostics(
    text: &str,
    model: &str,
    snapshot: &super::schemas::SchemaSnapshot,
    settings: &PerfSettings,
    dialect: &cogitator_dialects::Dialect,
) -> Vec<Diagnostic> {
    if settings.plan_needed() {
        return Vec::new();
    }
    let Some(checks) = snapshot.checks.get(model) else {
        return Vec::new();
    };
    if checks.text_hash != super::schemas::checks::text_hash(text) {
        return Vec::new(); // unsaved edits: the findings index another text
    }
    let suppressions = super::suppress::scan(text, dialect);
    let table = LineTable::new(text);
    let findings: Vec<FindingView> = checks
        .findings
        .iter()
        .map(FindingView::of)
        .map(|mut f| {
            f.suppressed = suppressions.suppresses(&table, f.id, f.span.start);
            f
        })
        .collect();
    finding_diagnostics(text, &table, &findings, settings)
}

pub(super) fn message(finding: &FindingView) -> String {
    match &finding.evidence {
        EvidenceView::Exact => finding.message.clone(),
        EvidenceView::Catalog { generated_at } => {
            format!("{} [catalog {generated_at}]", finding.message)
        }
        EvidenceView::Tested { .. } => format!("{} [dbt tests]", finding.message),
        EvidenceView::Heuristic => format!("{} [heuristic]", finding.message),
        EvidenceView::Unknown => format!("{} [unknown]", finding.message),
    }
}

pub(super) fn parse_severity(name: &str) -> Severity {
    match name {
        "warning" => Severity::Warning,
        "info" => Severity::Info,
        "hint" => Severity::Hint,
        _ => Severity::Off,
    }
}

fn to_lsp(severity: Severity) -> i32 {
    match severity {
        Severity::Warning => diagnostic_severity::WARNING,
        Severity::Info => diagnostic_severity::INFO,
        // Off is filtered before this; Hint is the safe floor
        Severity::Hint | Severity::Off => diagnostic_severity::HINT,
    }
}

/// The fixes of a finding as `Diagnostic.data`, preserving fixes first.
/// A fix with an unusable span is dropped alone; the diagnostic stands.
fn fix_payload(
    text: &str,
    table: &LineTable,
    finding: &FindingView,
    range: Range,
) -> Option<DiagnosticData> {
    let mut fixes: Vec<(bool, MultiEditFix)> = finding
        .fixes
        .iter()
        .filter_map(|fix| {
            let preserving = fix.safety == "preserving";
            Some((preserving, multi_edit(text, table, fix, preserving)?))
        })
        .collect();
    fixes.sort_by_key(|(preserving, _)| !preserving);
    if fixes.is_empty() {
        return None;
    }
    Some(DiagnosticData {
        fix: FixKind::MultiEdit {
            fixes: fixes.into_iter().map(|(_, fix)| fix).collect(),
        },
        token_range: range,
        suggestions: Vec::new(),
        subject: None,
    })
}

fn multi_edit(
    text: &str,
    table: &LineTable,
    fix: &FixView,
    preserving: bool,
) -> Option<MultiEditFix> {
    let mut edits = Vec::with_capacity(fix.edits.len());
    for edit in &fix.edits {
        let range = span_range(text, table, edit.span)?;
        edits.push(TextEdit {
            range,
            new_text: edit.new_text.clone(),
        });
    }
    let title = if preserving || fix.title.contains("changes results") {
        fix.title.clone()
    } else {
        format!("{} (changes results)", fix.title)
    };
    Some(MultiEditFix {
        title,
        edits,
        preserving,
    })
}

/// A byte span as an LSP range: lines from the table, columns in UTF-16
/// code units as the protocol's default encoding requires. `None` when
/// the span lies outside `text` or splits a character.
pub(crate) fn span_range(text: &str, table: &LineTable, span: Span) -> Option<Range> {
    if !span.is_valid_in(text) {
        return None;
    }
    let position = |offset: u32| {
        let (line, column) = table.line_col(offset);
        let line_start = offset - column;
        let character = text
            .get(line_start as usize..offset as usize)?
            .encode_utf16()
            .count() as u32;
        Some(Position { line, character })
    };
    Some(Range {
        start: position(span.start)?,
        end: position(span.end)?,
    })
}

#[cfg(test)]
#[path = "diagnostics_tests.rs"]
mod tests;
