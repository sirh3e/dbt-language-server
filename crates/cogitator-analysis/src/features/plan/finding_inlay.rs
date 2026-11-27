//! Plan findings as inlay hints (`perf.inline_findings`,
//! specs/ir-pipeline/08): a short `⚠ join.cartesian` at the end of the
//! first line of the finding's span, the full finding as its tooltip and a
//! label part that runs `dbt.queryPlan` for the model. Exactly the findings
//! the diagnostics report — same eligibility, suppression and severity
//! rules — so a rule that is off or below the profile shows no hint.
//!
//! Where a row-count hint sits on a finding's first line, within its span
//! (a finding on a CTE: the hint after its name), the two become ONE hint
//! at the row count's position, the row count first:
//! `≤ 1,200 rows · ⚠ model.unused_cte`. Several findings at one position
//! share a hint likewise, in plan order. Pure; the shell gates and plans.

use cogitator_ir::Span;
use cogitator_lsp::{
    inlay_hint_kind, AnyInlayHint, Command, InlayHint, InlayHintLabelPart, PartedInlayHint,
    Position, Range,
};
use cogitator_parser::LineTable;
use cogitator_rules::meta::Severity;

use super::diagnostics::{eligible, message, parse_severity, span_range};
use super::diagnostics_config::PerfSettings;
use super::report::{FindingView, QueryPlan};

/// Between two parts of one hint.
const SEPARATOR: &str = " · ";

/// One reported finding, placed: what its hint part shows and says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindingHint {
    /// Where the finding's span starts: a row hint between here and
    /// `position` (its CTE name) absorbs the finding.
    pub start: Position,
    pub position: Position,
    /// `⚠ join.cartesian`.
    pub label: String,
    /// Message (evidence-tagged as the diagnostic), rule, severity, fixes.
    pub tooltip: String,
}

/// The findings the diagnostics would report, as hints within `range`
/// (an all-zero range is the whole document).
pub fn finding_hints(
    text: &str,
    plan: &QueryPlan,
    settings: &PerfSettings,
    range: Range,
) -> Vec<FindingHint> {
    if !settings.inline_findings || !settings.active() {
        return Vec::new();
    }
    let table = LineTable::new(text);
    let whole = range.is_zero();
    plan.findings
        .iter()
        .filter(|f| !f.suppressed && eligible(f))
        .filter_map(|finding| {
            let severity = settings.severity(finding.id, parse_severity(finding.severity))?;
            let line = span_range(text, &table, first_line(text, finding.span)?)?;
            let at = line.end;
            (whole || (range.start.line <= at.line && at.line <= range.end.line)).then(|| {
                FindingHint {
                    start: line.start,
                    position: at,
                    label: format!("{} {}", icon(severity), finding.id),
                    tooltip: tooltip(finding, severity),
                }
            })
        })
        .collect()
}

/// The span cut to its first line, trailing blanks dropped: a hint after
/// a multi-line join sits after `cross join b`, not after the `on` clause.
fn first_line(text: &str, span: Span) -> Option<Span> {
    let body = text.get(span.start as usize..span.end as usize)?;
    let line = body.split('\n').next().unwrap_or(body).trim_end();
    if line.is_empty() {
        return Some(span);
    }
    Some(Span::new(span.start, span.start + line.len() as u32))
}

fn icon(severity: Severity) -> &'static str {
    match severity {
        Severity::Warning => "⚠",
        Severity::Info => "ℹ",
        Severity::Hint | Severity::Off => "💡",
    }
}

fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Warning => "warning",
        Severity::Info => "info",
        Severity::Hint | Severity::Off => "hint",
    }
}

fn tooltip(finding: &FindingView, severity: Severity) -> String {
    let mut out = format!(
        "{}\n\nrule: {} ({})",
        message(finding),
        finding.id,
        severity_name(severity)
    );
    for fix in &finding.fixes {
        out.push_str("\nsuggestion: ");
        out.push_str(&fix.title);
    }
    out
}

/// Row-count hints and finding hints as one list, ordered by position;
/// a row hint and the findings at its position become one parted hint.
pub fn merge_plan_hints(
    rows: Vec<InlayHint>,
    findings: Vec<FindingHint>,
    uri: &str,
) -> Vec<AnyInlayHint> {
    let mut groups: Vec<(Position, Vec<FindingHint>)> = Vec::new();
    for mut finding in findings {
        // a finding whose first line holds a row hint joins it there
        if let Some(row) = rows.iter().find(|r| covers(&finding, r.position)) {
            finding.position = row.position;
        }
        match groups.iter_mut().find(|(at, _)| *at == finding.position) {
            Some((_, group)) => group.push(finding),
            None => groups.push((finding.position, vec![finding])),
        }
    }
    let mut out: Vec<(Position, AnyInlayHint)> = Vec::new();
    for row in rows {
        let at = row.position;
        match groups.iter().position(|(p, _)| *p == at) {
            Some(i) => {
                let (_, group) = groups.remove(i);
                out.push((
                    at,
                    AnyInlayHint::Parted(parted(at, Some(row.label), group, uri)),
                ));
            }
            None => out.push((at, AnyInlayHint::Plain(row))),
        }
    }
    for (at, group) in groups {
        out.push((at, AnyInlayHint::Parted(parted(at, None, group, uri))));
    }
    out.sort_by_key(|(at, _)| (at.line, at.character));
    out.into_iter().map(|(_, hint)| hint).collect()
}

fn covers(finding: &FindingHint, at: Position) -> bool {
    let key = |p: Position| (p.line, p.character);
    at.line == finding.position.line
        && key(finding.start) <= key(at)
        && key(at) <= key(finding.position)
}

fn parted(
    position: Position,
    rows: Option<String>,
    findings: Vec<FindingHint>,
    uri: &str,
) -> PartedInlayHint {
    let mut label: Vec<InlayHintLabelPart> = Vec::new();
    let plain = |value: String| InlayHintLabelPart {
        value,
        ..Default::default()
    };
    if let Some(rows) = rows {
        label.push(plain(rows));
    }
    let tooltip = findings
        .iter()
        .map(|f| f.tooltip.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    for finding in findings {
        if !label.is_empty() {
            label.push(plain(SEPARATOR.to_string()));
        }
        label.push(InlayHintLabelPart {
            value: finding.label,
            tooltip: Some(finding.tooltip),
            command: Some(Command {
                title: "Show query plan".to_string(),
                command: "dbt.queryPlan".to_string(),
                arguments: vec![serde_json::json!({ "uri": uri })],
            }),
            location: None,
        });
    }
    PartedInlayHint {
        position,
        label,
        kind: Some(inlay_hint_kind::TYPE),
        padding_left: true,
        padding_right: false,
        tooltip: Some(tooltip),
    }
}

#[cfg(test)]
#[path = "finding_inlay_tests.rs"]
mod tests;
