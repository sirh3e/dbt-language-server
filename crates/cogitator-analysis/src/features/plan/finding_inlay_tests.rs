//! Case tables for `plan::finding_inlay`.

use cogitator_ir::lir::Backend;
use cogitator_ir::Level;
use cogitator_rules::meta::Needs;

use super::super::diagnostics_config::{PerfProfile, RuleOverride};
use super::super::report::{EvidenceView, FixView, LevelReport, Placement, PlanLevels, PlanStatus};
use super::super::trace::{NeedsView, TraceView};
use super::*;

const TEXT: &str = "with a as (select 1 as x)\nselect * from a\ncross join a b\n  on true\n";

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
        diagnostics_eligible: true,
        placement: Placement::Template,
        subject: None,
    }
}

fn plan(findings: Vec<FindingView>) -> QueryPlan {
    QueryPlan {
        model: Some("m".to_string()),
        dialect: "duckdb".to_string(),
        backend: Backend::DuckDb,
        schema_revision: 1,
        status: PlanStatus::Complete,
        guidance: None,
        banner: None,
        origin: None,
        levels: PlanLevels {
            hir: LevelReport::skipped("test"),
            mir: LevelReport::skipped("test"),
            lir: LevelReport::skipped("test"),
        },
        findings,
        trace: TraceView::unavailable(Level::Hir),
        needs: NeedsView::of(Needs::NONE),
        incrementality: None,
        row_hints: Vec::new(),
    }
}

fn on(profile: PerfProfile) -> PerfSettings {
    PerfSettings {
        profile,
        inline_findings: true,
        types_checks: true,
        ..PerfSettings::default()
    }
}

fn whole() -> Range {
    Range::default()
}

/// `cross join a b\n  on true` — two lines.
fn join_span() -> Span {
    let start = TEXT.find("cross").unwrap() as u32;
    Span::new(start, TEXT.find("true").unwrap() as u32 + 4)
}

fn labels(hints: &[FindingHint]) -> Vec<&str> {
    hints.iter().map(|h| h.label.as_str()).collect()
}

#[test]
fn the_same_rules_as_the_diagnostics_decide() {
    let mut suppressed = finding("join.cartesian", join_span(), "warning");
    suppressed.suppressed = true;
    let mut ineligible = finding("join.cartesian", join_span(), "warning");
    ineligible.diagnostics_eligible = false;
    let mut overridden = on(PerfProfile::Recommended);
    overridden
        .rules
        .insert("join.cartesian".to_string(), RuleOverride::Off);
    let toggled_off = PerfSettings {
        inline_findings: false,
        ..on(PerfProfile::Strict)
    };
    let cartesian = || finding("join.cartesian", join_span(), "warning");
    let cases: Vec<(&str, FindingView, PerfSettings, Vec<&str>)> = vec![
        (
            "reported",
            cartesian(),
            on(PerfProfile::Recommended),
            vec!["⚠ join.cartesian"],
        ),
        ("profile off", cartesian(), on(PerfProfile::Off), vec![]),
        ("toggle off", cartesian(), toggled_off, vec![]),
        ("rule off", cartesian(), overridden, vec![]),
        ("suppressed", suppressed, on(PerfProfile::Strict), vec![]),
        ("ineligible", ineligible, on(PerfProfile::Strict), vec![]),
        // a heuristic rule waits for strict, as its diagnostic does
        (
            "below the profile",
            finding("set.union_distinct", join_span(), "info"),
            on(PerfProfile::Recommended),
            vec![],
        ),
        (
            "info",
            finding("set.union_distinct", join_span(), "info"),
            on(PerfProfile::Strict),
            vec!["ℹ set.union_distinct"],
        ),
        (
            "outside the text",
            finding("join.cartesian", Span::new(900, 910), "warning"),
            on(PerfProfile::Strict),
            vec![],
        ),
    ];
    for (name, finding, settings, want) in cases {
        let hints = finding_hints(TEXT, &plan(vec![finding]), &settings, whole());
        assert_eq!(labels(&hints), want, "{name}");
    }
}

#[test]
fn a_hint_sits_after_the_first_line_and_tells_everything() {
    let mut found = finding("join.cartesian", join_span(), "warning");
    found.evidence = EvidenceView::Heuristic;
    found.fixes = vec![FixView {
        title: "Add a join key".to_string(),
        safety: "changesIntent",
        edits: Vec::new(),
    }];
    let hints = finding_hints(TEXT, &plan(vec![found]), &on(PerfProfile::Strict), whole());
    assert_eq!(hints.len(), 1);
    // `cross join a b` on line 2 ends at column 14, not after `on true`
    assert_eq!(
        hints[0].position,
        Position {
            line: 2,
            character: 14
        }
    );
    assert_eq!(
        hints[0].tooltip,
        "Join has no key [heuristic]\n\nrule: join.cartesian (warning)\nsuggestion: Add a join key"
    );
}

#[test]
fn a_range_keeps_only_its_lines() {
    let found = vec![finding("join.cartesian", join_span(), "warning")];
    let lines = |start, end| Range {
        start: Position {
            line: start,
            character: 0,
        },
        end: Position {
            line: end,
            character: 0,
        },
    };
    let settings = on(PerfProfile::Strict);
    assert_eq!(
        finding_hints(TEXT, &plan(found.clone()), &settings, lines(2, 3)).len(),
        1
    );
    assert!(finding_hints(TEXT, &plan(found), &settings, lines(0, 1)).is_empty());
}

fn row(line: u32, character: u32, label: &str) -> InlayHint {
    InlayHint {
        position: Position { line, character },
        label: label.to_string(),
        kind: Some(inlay_hint_kind::TYPE),
        padding_left: true,
        padding_right: false,
        tooltip: None,
    }
}

fn found(line: u32, character: u32, label: &str) -> FindingHint {
    FindingHint {
        start: Position { line, character },
        position: Position { line, character },
        label: label.to_string(),
        tooltip: format!("about {label}"),
    }
}

/// A hint's label as one string, parts joined.
fn shown(hint: &AnyInlayHint) -> String {
    match hint {
        AnyInlayHint::Plain(h) => h.label.clone(),
        AnyInlayHint::Parted(h) => h.label.iter().map(|p| p.value.as_str()).collect(),
    }
}

/// Name, row hints, finding hints, the labels wanted.
type MergeCase = (
    &'static str,
    Vec<InlayHint>,
    Vec<FindingHint>,
    Vec<&'static str>,
);

#[test]
fn rows_and_findings_merge_by_position() {
    let cases: Vec<MergeCase> = vec![
        (
            "rows alone stay plain",
            vec![row(0, 6, "3 rows")],
            vec![],
            vec!["3 rows"],
        ),
        (
            "one position, one hint, rows first",
            vec![row(0, 6, "≤ 1,200 rows")],
            vec![found(0, 6, "⚠ model.unused_cte")],
            vec!["≤ 1,200 rows · ⚠ model.unused_cte"],
        ),
        (
            "a row hint inside the finding's first line absorbs it",
            vec![row(0, 7, "3 rows")],
            vec![FindingHint {
                start: Position {
                    line: 0,
                    character: 2,
                },
                ..found(0, 20, "⚠ model.unused_cte")
            }],
            vec!["3 rows · ⚠ model.unused_cte"],
        ),
        (
            "a row hint before the finding stays apart",
            vec![row(0, 1, "3 rows")],
            vec![FindingHint {
                start: Position {
                    line: 0,
                    character: 2,
                },
                ..found(0, 20, "⚠ model.unused_cte")
            }],
            vec!["3 rows", "⚠ model.unused_cte"],
        ),
        (
            "findings at one position share a hint",
            vec![],
            vec![
                found(2, 14, "⚠ join.cartesian"),
                found(2, 14, "ℹ set.union_distinct"),
            ],
            vec!["⚠ join.cartesian · ℹ set.union_distinct"],
        ),
        (
            "apart, ordered by position",
            vec![row(0, 6, "3 rows")],
            vec![found(2, 14, "⚠ join.cartesian"), found(0, 2, "💡 x.y")],
            vec!["💡 x.y", "3 rows", "⚠ join.cartesian"],
        ),
    ];
    for (name, rows, findings, want) in cases {
        let merged = merge_plan_hints(rows, findings, "file:///m.sql");
        let got: Vec<String> = merged.iter().map(shown).collect();
        assert_eq!(got, want, "{name}");
    }
}

#[test]
fn a_finding_part_carries_its_tooltip_and_the_plan_command() {
    let merged = merge_plan_hints(
        vec![row(0, 6, "3 rows")],
        vec![found(0, 6, "⚠ model.unused_cte")],
        "file:///m.sql",
    );
    let AnyInlayHint::Parted(hint) = &merged[0] else {
        panic!("{merged:?}");
    };
    assert_eq!(hint.tooltip.as_deref(), Some("about ⚠ model.unused_cte"));
    let [rows, sep, part] = hint.label.as_slice() else {
        panic!("{:?}", hint.label);
    };
    assert!(rows.command.is_none() && sep.command.is_none() && rows.tooltip.is_none());
    assert_eq!(part.tooltip.as_deref(), Some("about ⚠ model.unused_cte"));
    let command = part.command.as_ref().expect("a command");
    assert_eq!(command.command, "dbt.queryPlan");
    assert_eq!(
        command.arguments,
        vec![serde_json::json!({"uri": "file:///m.sql"})]
    );
}
