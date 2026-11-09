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
use super::report::{EvidenceView, FindingView, FixView, OriginView, QueryPlan};
use super::suppress::Suppressions;

/// May this finding become a diagnostic at all? The eligibility rule:
/// its spans must map onto the document and be fresh. Compiled-origin
/// plans do not yet say so per finding, so only template-origin plans
/// qualify; this is the one line that changes when they do.
fn eligible(plan: &QueryPlan, _finding: &FindingView) -> bool {
    matches!(plan.origin, Some(OriginView::Template { .. }))
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
    let mut out = Vec::new();
    for finding in plan.findings.iter().filter(|f| !f.suppressed) {
        if !eligible(plan, finding) {
            continue;
        }
        let Some(severity) = settings.severity(finding.id, parse_severity(finding.severity)) else {
            continue;
        };
        let Some(range) = span_range(text, &table, finding.span) else {
            tracing::debug!(rule = finding.id, "finding span is outside the text; dropped");
            continue;
        };
        let mut diagnostic = Diagnostic::plain(
            range,
            message(finding),
            to_lsp(severity),
            finding.id,
            "cogitator",
        );
        diagnostic.data = fix_payload(text, &table, finding, range);
        out.push(diagnostic);
    }
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

fn message(finding: &FindingView) -> String {
    match &finding.evidence {
        EvidenceView::Exact => finding.message.clone(),
        EvidenceView::Catalog { generated_at } => {
            format!("{} [catalog {generated_at}]", finding.message)
        }
        EvidenceView::Heuristic => format!("{} [heuristic]", finding.message),
        EvidenceView::Unknown => format!("{} [unknown]", finding.message),
    }
}

fn parse_severity(name: &str) -> Severity {
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
mod tests {
    use cogitator_ir::lir::Backend;
    use cogitator_ir::origin::{Staleness, TemplateVariant};
    use cogitator_ir::Level;

    use super::super::diagnostics_config::{PerfProfile, RuleOverride};
    use super::super::report::{FixEditView, LevelReport, PlanLevels, PlanStatus};
    use super::super::suppress::scan;
    use super::super::trace::{NeedsView, TraceView};
    use super::*;
    use cogitator_dialects::Dialect;
    use cogitator_rules::meta::Needs;

    fn finding(id: &'static str, span: Span, severity: &'static str) -> FindingView {
        FindingView {
            id,
            span,
            level: Level::Mir,
            severity,
            message: "Join has no key".to_string(),
            evidence: EvidenceView::Exact,
            fixes: Vec::new(),
            suppressed: false,
        }
    }

    fn plan(findings: Vec<FindingView>, origin: Option<OriginView>) -> QueryPlan {
        QueryPlan {
            model: Some("m".to_string()),
            dialect: "duckdb".to_string(),
            backend: Backend::DuckDb,
            schema_revision: 1,
            status: PlanStatus::Complete,
            guidance: None,
            origin,
            levels: PlanLevels {
                hir: LevelReport::skipped("test"),
                mir: LevelReport::skipped("test"),
                lir: LevelReport::skipped("test"),
            },
            findings,
            trace: TraceView::unavailable(Level::Hir),
            needs: NeedsView::of(Needs::NONE),
        }
    }

    fn template() -> Option<OriginView> {
        Some(OriginView::Template {
            variant: TemplateVariant::Plain,
        })
    }

    fn settings(profile: PerfProfile) -> PerfSettings {
        PerfSettings {
            profile,
            ..PerfSettings::default()
        }
    }

    fn run(text: &str, findings: Vec<FindingView>, cfg: &PerfSettings) -> Vec<Diagnostic> {
        let plan = plan(findings, template());
        plan_diagnostics(
            text,
            &plan,
            &scan(text, &Dialect::new("duckdb")),
            cfg,
        )
    }

    #[test]
    fn profile_off_produces_nothing() {
        let found = vec![finding("join.cartesian", Span::new(0, 6), "warning")];
        assert!(run("select 1\n", found, &PerfSettings::default()).is_empty());
    }

    #[test]
    fn recommended_produces_a_diagnostic() {
        let found = vec![finding("join.cartesian", Span::new(0, 6), "warning")];
        let diags = run("select 1\n", found, &settings(PerfProfile::Recommended));
        assert_eq!(diags.len(), 1);
        let d = &diags[0];
        assert_eq!(d.code, "join.cartesian");
        assert_eq!(d.source, "cogitator");
        assert_eq!(d.severity, diagnostic_severity::WARNING);
        assert_eq!(d.message, "Join has no key");
        assert_eq!(d.range.end.character, 6);
    }

    #[test]
    fn evidence_tags_the_message_unless_exact() {
        let mut heuristic = finding("join.cartesian", Span::new(0, 6), "warning");
        heuristic.evidence = EvidenceView::Heuristic;
        let mut catalog = finding("join.left_nullified", Span::new(0, 6), "warning");
        catalog.evidence = EvidenceView::Catalog {
            generated_at: "2024".to_string(),
        };
        let diags = run(
            "select 1\n",
            vec![heuristic, catalog],
            &settings(PerfProfile::Strict),
        );
        assert_eq!(diags[0].message, "Join has no key [heuristic]");
        assert_eq!(diags[1].message, "Join has no key [catalog 2024]");
    }

    #[test]
    fn dialect_severity_stays_and_overrides_win() {
        // filter.non_sargable reports Hint off BigQuery
        let found = vec![finding("filter.non_sargable", Span::new(0, 6), "hint")];
        let on = run("select 1\n", found.clone(), &settings(PerfProfile::Recommended));
        assert_eq!(on[0].severity, diagnostic_severity::HINT);
        let mut cfg = settings(PerfProfile::Recommended);
        cfg.rules.insert(
            "filter.non_sargable".to_string(),
            RuleOverride::Set(Severity::Warning),
        );
        assert_eq!(run("select 1\n", found.clone(), &cfg)[0].severity, 2);
        cfg.rules
            .insert("filter.non_sargable".to_string(), RuleOverride::Off);
        assert!(run("select 1\n", found, &cfg).is_empty());
    }

    #[test]
    fn stats_evidence_cannot_raise_severity() {
        // a catalog-backed finding at the rule's ceiling stays there; a
        // stale value claiming more is capped, never reported above it
        let mut boosted = finding("join.distinct_fanout", Span::new(0, 6), "warning");
        boosted.evidence = EvidenceView::Catalog {
            generated_at: "2024".to_string(),
        };
        let diags = run("select 1\n", vec![boosted], &settings(PerfProfile::Strict));
        assert_eq!(diags[0].severity, diagnostic_severity::INFO);
    }

    #[test]
    fn ineligible_plans_produce_nothing() {
        let compiled = Some(OriginView::Compiled {
            staleness: Staleness::Fresh,
        });
        let plan = plan(
            vec![finding("join.cartesian", Span::new(0, 6), "warning")],
            compiled,
        );
        assert!(plan_diagnostics(
            "select 1\n",
            &plan,
            &Suppressions::default(),
            &settings(PerfProfile::Strict),
        )
        .is_empty());
    }

    #[test]
    fn multibyte_text_converts_to_utf16_columns() {
        // é is two bytes, one UTF-16 unit; 𝄞 is four bytes, two units
        let text = "select 'é𝄞', x\n";
        let start = text.find('x').unwrap() as u32;
        let found = vec![finding("join.cartesian", Span::new(start, start + 1), "warning")];
        let diags = run(text, found, &settings(PerfProfile::Recommended));
        assert_eq!(diags[0].range.start.character, 14, "utf-16 column of x");
        assert_eq!(diags[0].range.end.character, 15);
    }

    #[test]
    fn spans_outside_the_text_are_dropped() {
        let found = vec![finding("join.cartesian", Span::new(0, 400), "warning")];
        assert!(run("select 1\n", found, &settings(PerfProfile::Strict)).is_empty());
    }

    #[test]
    fn fixes_become_multi_edit_preserving_first() {
        let mut f = finding("set.union_distinct", Span::new(0, 6), "info");
        f.fixes = vec![
            FixView {
                title: "Move it".to_string(),
                safety: "changesIntent",
                edits: vec![FixEditView {
                    span: Span::new(0, 6),
                    new_text: "x".to_string(),
                }],
            },
            FixView {
                title: "Change it".to_string(),
                safety: "preserving",
                edits: vec![
                    FixEditView {
                        span: Span::new(0, 0),
                        new_text: "a".to_string(),
                    },
                    FixEditView {
                        span: Span::new(1, 2),
                        new_text: String::new(),
                    },
                ],
            },
        ];
        let diags = run("select 1\n", vec![f], &settings(PerfProfile::Strict));
        let data = diags[0].data.clone().expect("fixes");
        let FixKind::MultiEdit { fixes } = data.fix else {
            panic!("multi edit");
        };
        assert_eq!(fixes[0].title, "Change it");
        assert!(fixes[0].preserving);
        assert_eq!(fixes[0].edits.len(), 2, "edits apply together");
        assert_eq!(fixes[0].edits[0].range.start, fixes[0].edits[0].range.end);
        assert_eq!(fixes[1].title, "Move it (changes results)");
        assert!(!fixes[1].preserving);
    }

    #[test]
    fn a_fix_with_a_bad_edit_is_dropped_alone() {
        let mut f = finding("join.cartesian", Span::new(0, 6), "warning");
        f.fixes = vec![FixView {
            title: "t".to_string(),
            safety: "preserving",
            edits: vec![FixEditView {
                span: Span::new(0, 400),
                new_text: String::new(),
            }],
        }];
        let diags = run("select 1\n", vec![f], &settings(PerfProfile::Strict));
        assert_eq!(diags.len(), 1);
        assert!(diags[0].data.is_none());
    }

    #[test]
    fn suppressed_findings_are_silent_and_unknown_ids_warn() {
        let text = "select 1 -- cogitator: ignore join.cartesian\n\
                    select 2 -- cogitator: ignore join.cartesan\n";
        let mut suppressed = finding("join.cartesian", Span::new(0, 6), "warning");
        suppressed.suppressed = true;
        let diags = run(text, vec![suppressed], &settings(PerfProfile::Strict));
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "cogitator-suppress");
        assert!(diags[0].message.contains("join.cartesian"), "{}", diags[0].message);
        assert_eq!(diags[0].severity, diagnostic_severity::WARNING);
    }

    #[test]
    fn suppression_warnings_wait_for_opt_in() {
        let text = "select 1 -- cogitator: ignore join.cartesan\n";
        assert!(run(text, Vec::new(), &PerfSettings::default()).is_empty());
    }
}
