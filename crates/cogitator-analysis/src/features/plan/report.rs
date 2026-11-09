//! The `dbt.queryPlan` payload (specs/ir-pipeline/09). The pass manager's
//! trace and findings are not `Serialize`, so these views own the wire
//! vocabulary.

use cogitator_ir::lir::Backend;
use cogitator_ir::origin::{SourceOrigin, Staleness, TemplateVariant};
use cogitator_ir::{Degradation, Fidelity, Invariant, Level, Lowered, Span, Violation};
use cogitator_rules::findings::{Evidence, Finding, Fix};
use cogitator_rules::meta::{FixSafety, Need, Needs, Severity};
use cogitator_rules::passes::{Outcome, Phase, SkipReason, Trace};
use serde::Serialize;

use super::PlanInput;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PlanStatus {
    Complete,
    Partial,
    Declined,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OriginView {
    Template { variant: TemplateVariant },
    Compiled { staleness: Staleness },
}

impl OriginView {
    pub(super) fn of(origin: &SourceOrigin) -> Self {
        match origin {
            SourceOrigin::Template { variant } => OriginView::Template { variant: *variant },
            SourceOrigin::Compiled { staleness, .. } => OriginView::Compiled {
                staleness: *staleness,
            },
        }
    }
}

/// One IR level: its printed form or why there is none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelReport {
    pub available: bool,
    /// `None` for a level the pipeline never reached.
    pub fidelity: Option<Fidelity>,
    pub text: Option<String>,
    /// Why the level is unavailable; names its violations or degradation.
    pub unavailable: Option<String>,
    pub degradations: Vec<Degradation>,
    pub violations: Vec<Violation>,
}

impl LevelReport {
    /// A level the pipeline never reached because `above` was dropped.
    pub(super) fn skipped(above: &str) -> Self {
        LevelReport {
            available: false,
            fidelity: None,
            text: None,
            unavailable: Some(format!("not lowered: {above}")),
            degradations: Vec::new(),
            violations: Vec::new(),
        }
    }

    /// `text` is the level's printer output when it survived verification.
    pub(super) fn of<T>(lowered: &Lowered<T>, text: Option<String>, name: &str) -> Self {
        let available = lowered.value.is_some();
        LevelReport {
            available,
            fidelity: Some(lowered.fidelity),
            text: if available { text } else { None },
            unavailable: (!available).then(|| unavailable_note(lowered, name)),
            degradations: lowered.degraded.clone(),
            violations: lowered.violations.clone(),
        }
    }
}

fn unavailable_note<T>(lowered: &Lowered<T>, name: &str) -> String {
    if lowered.violations.is_empty() {
        let detail = lowered
            .degraded
            .first()
            .map_or("no plan was produced", |d| d.detail.as_str());
        format!("{name} declined: {detail}")
    } else {
        let invariants: Vec<String> = lowered
            .violations
            .iter()
            .map(|v| format!("{:?}", v.invariant))
            .collect();
        format!("{name} dropped by the verifier: {}", invariants.join(", "))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlanLevels {
    pub hir: LevelReport,
    pub mir: LevelReport,
    pub lir: LevelReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EvidenceView {
    Exact,
    #[serde(rename_all = "camelCase")]
    Catalog {
        generated_at: String,
    },
    Heuristic,
    Unknown,
}

impl EvidenceView {
    fn of(evidence: &Evidence) -> Self {
        match evidence {
            Evidence::Exact => EvidenceView::Exact,
            Evidence::Catalog { generated_at } => EvidenceView::Catalog {
                generated_at: generated_at.to_string(),
            },
            Evidence::Heuristic => EvidenceView::Heuristic,
            Evidence::Unknown => EvidenceView::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FixEditView {
    pub span: Span,
    pub new_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FixView {
    pub title: String,
    pub safety: &'static str,
    pub edits: Vec<FixEditView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FindingView {
    pub id: &'static str,
    pub span: Span,
    pub level: Level,
    pub severity: &'static str,
    pub message: String,
    pub evidence: EvidenceView,
    pub fixes: Vec<FixView>,
}

fn severity(value: Severity) -> &'static str {
    match value {
        Severity::Off => "off",
        Severity::Hint => "hint",
        Severity::Info => "info",
        Severity::Warning => "warning",
    }
}

fn safety(value: FixSafety) -> &'static str {
    match value {
        FixSafety::None => "none",
        FixSafety::Preserving => "preserving",
        FixSafety::ChangesIntent => "changesIntent",
        FixSafety::Mixed => "mixed",
    }
}

fn fix_view(fix: &Fix) -> FixView {
    FixView {
        title: fix.title.clone(),
        safety: safety(fix.safety),
        edits: fix
            .edits
            .iter()
            .map(|edit| FixEditView {
                span: edit.span,
                new_text: edit.new_text.clone(),
            })
            .collect(),
    }
}

impl FindingView {
    pub(super) fn of(finding: &Finding) -> Self {
        FindingView {
            id: finding.id.as_str(),
            span: finding.span,
            level: finding.level,
            severity: severity(finding.severity),
            message: finding.message.clone(),
            evidence: EvidenceView::of(&finding.evidence),
            fixes: finding.fixes.iter().map(fix_view).collect(),
        }
    }
}

/// One rule visit. `skipped` rows always carry a `reason`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceRow {
    pub rule: &'static str,
    pub phase: &'static str,
    pub pass: u32,
    /// `applied`, `noChange`, `findings`, `skipped` or `rolledBack`.
    pub outcome: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub invariants: Vec<Invariant>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TraceView {
    pub entries: Vec<TraceRow>,
    /// A budget ran out, or the plan could not be verified.
    pub incomplete: bool,
}

pub(super) fn phase_name(phase: Phase) -> &'static str {
    match phase {
        Phase::Canonicalize => "canonicalize",
        Phase::DetectHir => "detectHir",
        Phase::DetectMir => "detectMir",
        Phase::DetectLir => "detectLir",
    }
}

fn need_name(need: Need) -> &'static str {
    match need {
        Need::Schema => "schema",
        Need::Types => "types",
        Need::Stats => "stats",
        Need::Dialect => "dialect",
        Need::Mat => "mat",
        Need::Dag => "dag",
        Need::Tests => "tests",
        Need::Phys => "phys",
        Need::AllBranches => "allBranches",
    }
}

pub(super) fn skip_reason(reason: SkipReason) -> String {
    match reason {
        SkipReason::Unmet(need) => format!("unmet:{}", need_name(need)),
        SkipReason::Budget => "budget".to_string(),
        SkipReason::Unavailable(level) => format!("unavailable:{level:?}").to_lowercase(),
        SkipReason::InvalidPlan => "invalidPlan".to_string(),
    }
}

fn row(rule: &'static str, phase: Phase, pass: u32, outcome: &Outcome) -> TraceRow {
    let mut row = TraceRow {
        rule,
        phase: phase_name(phase),
        pass,
        outcome: "applied",
        count: None,
        reason: None,
        invariants: Vec::new(),
    };
    match outcome {
        Outcome::Applied => {}
        Outcome::NoChange => row.outcome = "noChange",
        Outcome::Findings { count } => {
            row.outcome = "findings";
            row.count = Some(*count);
        }
        Outcome::Skipped { reason } => {
            row.outcome = "skipped";
            row.reason = Some(skip_reason(*reason));
        }
        Outcome::RolledBack { invariants } => {
            row.outcome = "rolledBack";
            row.invariants = invariants.clone();
        }
    }
    row
}

impl TraceView {
    pub(super) fn of(trace: &Trace) -> Self {
        TraceView {
            entries: trace
                .entries
                .iter()
                .map(|e| row(e.rule.as_str(), e.phase, e.pass, &e.outcome))
                .collect(),
            incomplete: trace.incomplete,
        }
    }

    /// Every rule skipped because `level` is unavailable, in run order.
    pub(super) fn unavailable(level: Level) -> Self {
        let reason = Outcome::Skipped {
            reason: SkipReason::Unavailable(level),
        };
        let mut entries = Vec::new();
        for rule in cogitator_rules::registry::REGISTRY {
            let id = rule.meta.id.as_str();
            let phases = [
                (rule.canon.is_some(), Phase::Canonicalize),
                (rule.hir.is_some(), Phase::DetectHir),
                (rule.mir.is_some(), Phase::DetectMir),
                (rule.lir.is_some(), Phase::DetectLir),
            ];
            for (_, phase) in phases.iter().filter(|(present, _)| *present) {
                entries.push(row(id, *phase, 0, &reason));
            }
        }
        TraceView {
            entries,
            incomplete: true,
        }
    }
}

/// What the plan could establish, named for the plan view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NeedsView {
    pub available: Vec<&'static str>,
    pub missing: Vec<&'static str>,
}

impl NeedsView {
    pub(super) fn of(available: Needs) -> Self {
        let mut view = NeedsView {
            available: Vec::new(),
            missing: Vec::new(),
        };
        for need in [
            Need::Schema,
            Need::Types,
            Need::Stats,
            Need::Dialect,
            Need::Mat,
            Need::Dag,
            Need::Tests,
            Need::Phys,
            Need::AllBranches,
        ] {
            let list = if available.contains(need) {
                &mut view.available
            } else {
                &mut view.missing
            };
            list.push(need_name(need));
        }
        view
    }
}

/// The `dbt.queryPlan` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryPlan {
    pub model: Option<String>,
    pub dialect: String,
    pub backend: Backend,
    pub schema_revision: u64,
    pub status: PlanStatus,
    /// Set when `status` is `declined`: what to do about it.
    pub guidance: Option<String>,
    pub origin: Option<OriginView>,
    pub levels: PlanLevels,
    pub findings: Vec<FindingView>,
    pub trace: TraceView,
    pub needs: NeedsView,
}

impl QueryPlan {
    pub(super) fn no_backend(input: &PlanInput<'_>, backend: Backend) -> Self {
        let skipped = LevelReport::skipped("no backend for this dialect");
        QueryPlan {
            model: input.model.map(str::to_string),
            dialect: input.dialect.as_str().to_string(),
            backend,
            schema_revision: input.schema_revision,
            status: PlanStatus::Declined,
            guidance: None,
            origin: None,
            levels: PlanLevels {
                hir: skipped.clone(),
                mir: skipped.clone(),
                lir: skipped,
            },
            findings: Vec::new(),
            trace: TraceView::unavailable(Level::Hir),
            needs: NeedsView::of(Needs::NONE),
        }
    }
}
