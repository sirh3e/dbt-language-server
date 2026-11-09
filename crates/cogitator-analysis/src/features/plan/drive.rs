//! The staged driver: text → HIR → MIR → passes (with LIR lowered through
//! `PassInput::lower_lir`) → report. Each stage stops the ones below it
//! when it produces nothing; the report says which and why.

use std::cell::RefCell;

use cogitator_error::guidance::Guidance;
use cogitator_ir::common::Level;
use cogitator_ir::hir::{print_hir, Hir};
use cogitator_ir::lir::{print_lir, Backend, Lir};
use cogitator_ir::mir::{print_mir, Mir};
use cogitator_ir::origin::{SourceOrigin, TemplateVariant};
use cogitator_ir::{DegradationKind, Fidelity, Lowered, VerifyMode, Violation};
use cogitator_lower::{lower_to_lir, DialectBackend};
use cogitator_rules::cx::Cx;
use cogitator_rules::meta::{Need, Needs};
use cogitator_rules::passes::{run_passes, Budgets, LirLowering, PassInput};
use cogitator_sql_front::{build_hir, HirInput};

use super::env::{bare_env, fill, BuiltEnv, Upstreams};
use super::facts::merge_manifest;
use super::phys::{physical_facts, BuiltPhys};
use super::report::{
    FindingView, LevelReport, NeedsView, OriginView, PlanLevels, PlanStatus, QueryPlan, TraceView,
};
use super::PlanInput;

pub(super) type MirLowering<'a> = &'a dyn Fn(&Hir) -> Lowered<Mir>;

/// Runs the whole pipeline. `lower` is the HIR→MIR seam tests use to force
/// a violation; everywhere else it is `lower_to_mir`.
pub(super) fn drive(
    input: &PlanInput<'_>,
    backend: &dyn DialectBackend,
    lower: MirLowering<'_>,
) -> QueryPlan {
    let built = built_hir(input);
    let mut hir_lowered = built.hir;
    if let Some(hir) = hir_lowered.value.as_mut() {
        merge_manifest_config(hir, input);
    }
    log_violations(Level::Hir, &hir_lowered.violations);
    let hir_text = hir_lowered.value.as_ref().map(print_hir);
    let hir_report = LevelReport::of(&hir_lowered, hir_text, "HIR");
    let backend_kind = backend.capabilities().backend;

    let Some(hir) = hir_lowered.value.as_ref() else {
        return declined(input, &hir_lowered, hir_report, backend_kind);
    };

    let mir_lowered = lower(hir);
    log_violations(Level::Mir, &mir_lowered.violations);
    let Lowered {
        value: mir,
        fidelity: mir_fidelity,
        degraded: mir_degraded,
        violations: mir_violations,
    } = mir_lowered;
    let Some(mir) = mir else {
        let lowered: Lowered<Mir> = Lowered {
            value: None,
            fidelity: mir_fidelity,
            degraded: mir_degraded,
            violations: mir_violations,
        };
        return without_mir(input, hir, hir_report, &lowered, backend_kind);
    };

    let run = Passes {
        input,
        backend,
        env: built.env,
    }
    .run(hir, mir);

    let mir_report = LevelReport {
        available: true,
        fidelity: Some(mir_fidelity),
        text: Some(print_mir(&run.mir)),
        unavailable: None,
        degradations: mir_degraded,
        violations: mir_violations,
    };
    let lir_report = lir_report(&run);
    let status = status_of(&hir_report, &mir_report, &lir_report, run.trace.incomplete);
    QueryPlan {
        model: input.model.map(str::to_string),
        dialect: input.dialect.as_str().to_string(),
        backend: backend_kind,
        schema_revision: input.schema_revision,
        status,
        guidance: None,
        origin: Some(OriginView::of(&hir.origin)),
        levels: PlanLevels {
            hir: hir_report,
            mir: mir_report,
            lir: lir_report,
        },
        findings: run.findings,
        trace: run.trace,
        needs: run.needs,
    }
}

struct BuiltHir {
    hir: Lowered<Hir>,
    env: BuiltEnv,
}

fn hir_input<'a>(input: &PlanInput<'a>) -> HirInput<'a> {
    HirInput {
        text: input.text,
        dialect: input.dialect,
        origin: SourceOrigin::Template {
            variant: TemplateVariant::Plain,
        },
        config: Default::default(),
    }
}

/// HIR is built once to learn the direct upstreams from the live text,
/// and again only when those upstreams have schema facts to bind against.
fn built_hir(input: &PlanInput<'_>) -> BuiltHir {
    let front = hir_input(input);
    let first = build_hir(&front, &bare_env(input));
    let Some(hir) = first.value.as_ref() else {
        return BuiltHir {
            hir: first,
            env: BuiltEnv::default(),
        };
    };
    let env = fill(&Upstreams::of(hir), input);
    if env.is_empty() {
        return BuiltHir { hir: first, env };
    }
    BuiltHir {
        hir: build_hir(&front, &env.env),
        env,
    }
}

fn merge_manifest_config(hir: &mut Hir, input: &PlanInput<'_>) {
    let manifest = input
        .model
        .and_then(|m| input.manifest?.models.get(m))
        .map(|node| &node.config);
    let contract = input
        .model
        .and_then(|m| input.models.get(m))
        .is_some_and(|details| details.contract_enforced);
    merge_manifest(
        &mut hir.config,
        &mut hir.syms,
        manifest,
        contract,
        input.dialect,
    );
}

fn log_violations(level: Level, violations: &[Violation]) {
    for violation in violations {
        tracing::error!(?level, invariant = ?violation.invariant, detail = %violation.detail,
            "IR verifier violation; level dropped");
    }
}

/// HIR produced nothing: control flow the template cannot express, or a
/// verifier violation.
fn declined(
    input: &PlanInput<'_>,
    lowered: &Lowered<Hir>,
    hir: LevelReport,
    backend: Backend,
) -> QueryPlan {
    QueryPlan {
        model: input.model.map(str::to_string),
        dialect: input.dialect.as_str().to_string(),
        backend,
        schema_revision: input.schema_revision,
        status: PlanStatus::Declined,
        guidance: Some(guidance(input, lowered).to_message()),
        origin: None,
        levels: PlanLevels {
            hir,
            mir: LevelReport::skipped("the HIR is unavailable"),
            lir: LevelReport::skipped("the HIR is unavailable"),
        },
        findings: Vec::new(),
        trace: TraceView::unavailable(Level::Hir),
        needs: NeedsView::of(Needs::NONE),
    }
}

fn guidance(input: &PlanInput<'_>, lowered: &Lowered<Hir>) -> Guidance {
    let model = input.model.unwrap_or("this model");
    if let Some(violation) = lowered.violations.first() {
        return Guidance::new(format!(
            "No query plan for {model}: the verifier rejected it."
        ))
        .because(format!("{:?}: {}", violation.invariant, violation.detail))
        .fix("this is a cogitator bug; please report it with the model's SQL.");
    }
    let reason = lowered
        .degraded
        .iter()
        .find(|d| d.kind == DegradationKind::JinjaControlFlow)
        .or(lowered.degraded.first())
        .map_or("the template could not be lowered", |d| d.detail.as_str());
    Guidance::new(format!("No query plan for {model} from its template."))
        .because(reason.to_string())
        .fix("run `dbt compile` and inspect the compiled SQL; plans from compiled output are not available yet.")
}

/// MIR lowering produced nothing. HIR is shown; everything below is not.
fn without_mir(
    input: &PlanInput<'_>,
    hir: &Hir,
    hir_report: LevelReport,
    lowered: &Lowered<Mir>,
    backend: Backend,
) -> QueryPlan {
    QueryPlan {
        model: input.model.map(str::to_string),
        dialect: input.dialect.as_str().to_string(),
        backend,
        schema_revision: input.schema_revision,
        status: PlanStatus::Partial,
        guidance: None,
        origin: Some(OriginView::of(&hir.origin)),
        levels: PlanLevels {
            hir: hir_report,
            mir: LevelReport::of(lowered, None, "MIR"),
            lir: LevelReport::skipped("the MIR is unavailable"),
        },
        findings: Vec::new(),
        trace: TraceView::unavailable(Level::Mir),
        needs: NeedsView::of(Needs::NONE),
    }
}

/// The output of the pass manager, in report vocabulary.
struct Run {
    mir: Mir,
    lir: Option<Lir>,
    /// What `lower_lir` reported; `None` when the hook never ran.
    lir_outcome: Option<LirOutcome>,
    findings: Vec<FindingView>,
    trace: TraceView,
    needs: NeedsView,
}

struct LirOutcome {
    fidelity: Fidelity,
    degraded: Vec<cogitator_ir::Degradation>,
    violations: Vec<Violation>,
}

struct Passes<'a, 'i> {
    input: &'a PlanInput<'i>,
    backend: &'a dyn DialectBackend,
    env: BuiltEnv,
}

impl Passes<'_, '_> {
    fn run(self, hir: &Hir, mut mir: Mir) -> Run {
        let Passes {
            input,
            backend,
            env: built,
        } = self;
        let caps = backend.capabilities();
        let schemas = built.env.schemas.clone();
        let phys = physical_facts(&mut mir, &schemas, caps.pruning_keys, input.model, input);
        let available = needs(input, hir, &built, &phys, caps.backend);

        let outcome: RefCell<Option<LirOutcome>> = RefCell::new(None);
        // `lower_to_lir` runs `verify_lir` against this backend's table.
        let lower = |canonical: &Mir| -> Option<Lir> {
            let mut lowered = lower_to_lir(canonical, backend, &phys.facts);
            log_violations(Level::Lir, &lowered.violations);
            let lir = lowered.value.take();
            *outcome.borrow_mut() = Some(LirOutcome {
                fidelity: lowered.fidelity,
                degraded: std::mem::take(&mut lowered.degraded),
                violations: std::mem::take(&mut lowered.violations),
            });
            lir
        };
        let cx = Cx {
            available,
            hir: Some(hir),
            mir: None,
            dialect: Some(caps.backend),
            mat: hir.config.materialized,
            config: Some(&hir.config),
            schemas: built.schema_known.then_some(&*schemas),
            phys: phys.has_keys.then_some(&phys.facts),
            dag: None,
        };
        let output = run_passes(
            PassInput {
                hir: Some(hir),
                mir,
                lir: None,
                lower_lir: Some(LirLowering(&lower)),
                verify: VerifyMode::for_build(),
                budgets: Budgets::default(),
                snapshots: false,
            },
            &cx,
        );
        let trace = TraceView::of(&output.trace);
        Run {
            findings: output.findings.iter().map(FindingView::of).collect(),
            trace,
            mir: output.mir,
            lir: output.lir,
            lir_outcome: outcome.into_inner(),
            needs: NeedsView::of(available),
        }
    }
}

/// `Need::Dialect` is held only by a named backend: `generic` has no
/// capability table to reason with. `Tests` and `Dag` have no facts yet.
fn needs(
    input: &PlanInput<'_>,
    hir: &Hir,
    env: &BuiltEnv,
    phys: &BuiltPhys,
    backend: Backend,
) -> Needs {
    let mut needs = Needs::NONE;
    let mut set = |need: Need, holds: bool| {
        if holds {
            needs = needs.with(need);
        }
    };
    set(Need::Schema, env.schema_known);
    set(Need::Types, env.types_known);
    set(Need::Stats, phys.has_stats);
    set(Need::Phys, phys.has_keys);
    set(Need::Mat, hir.config.materialized.is_some());
    set(
        Need::Dialect,
        !input.dialect.as_str().is_empty() && backend != Backend::Generic,
    );
    // a template shows every branch; a compiled plan shows one
    set(
        Need::AllBranches,
        matches!(hir.origin, SourceOrigin::Template { .. }),
    );
    needs
}

fn lir_report(run: &Run) -> LevelReport {
    let Some(outcome) = &run.lir_outcome else {
        return LevelReport::skipped("the pass manager stopped before LIR");
    };
    let lowered = Lowered {
        value: run.lir.as_ref(),
        fidelity: outcome.fidelity,
        degraded: outcome.degraded.clone(),
        violations: outcome.violations.clone(),
    };
    let text = run.lir.as_ref().map(|lir| print_lir(lir, &run.mir));
    LevelReport::of(&lowered, text, "LIR")
}

fn status_of(
    hir: &LevelReport,
    mir: &LevelReport,
    lir: &LevelReport,
    incomplete: bool,
) -> PlanStatus {
    let clean = |level: &LevelReport| {
        level.available
            && level.fidelity == Some(Fidelity::Complete)
            && level.degradations.is_empty()
            && level.violations.is_empty()
    };
    if clean(hir) && clean(mir) && clean(lir) && !incomplete {
        PlanStatus::Complete
    } else {
        PlanStatus::Partial
    }
}
