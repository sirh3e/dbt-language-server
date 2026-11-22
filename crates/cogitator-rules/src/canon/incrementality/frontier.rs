//! The incrementality frontier (specs/ir-pipeline/10 §6): when a model has
//! to stay a table, the parts of it that would not — the CTEs whose body,
//! taken as a model of its own, is incremental. Materializing such a part
//! as an incremental model turns `Q = Q_keep ∘ Q_inc` into `Q_keep ∘ T`:
//! `Q_inc` then does work in proportion to the new rows, and `Q_keep`
//! still rebuilds in full, but over `T`.
//!
//! That pays only when `T` is smaller than what `Q_inc` reads: the body
//! must aggregate, deduplicate or filter (`reduces`). A part that only
//! projects its input is refused, as is a CTE that reads another CTE of
//! the model (cut out as text, it would reference nothing). Cuts are text
//! boundaries only: everything written in one SELECT is refused as
//! `no-boundary`.

use std::collections::BTreeSet;

use cogitator_ir::mir::{CteId, Mir, RelId, RelOp};
use cogitator_ir::sexp::{atom, list, string, Sexp};

use super::annotate::verdict_sexp;
use super::{classify, classify_at, Analysis, IncrementalInputs, Verdict};

/// One CTE that could become an incremental model of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cut {
    pub cte: String,
    pub body: RelId,
    /// The body's own analysis, as a model.
    pub analysis: Analysis,
    /// What makes the materialized part smaller than its input.
    pub reduces: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The part only projects its input: materialized, it is as large as
    /// what it reads, so nothing is saved.
    NoReduction { cte: Option<String> },
    /// The growing work is in the same SELECT as what blocks it.
    NoBoundary,
    /// The CTE reads another CTE of the model.
    ReadsCte { cte: String },
    /// The CTE is not incremental itself.
    NotIncremental { cte: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Frontier {
    pub cuts: Vec<Cut>,
    pub refusals: Vec<Refusal>,
}

/// The frontier of a model whose own verdict keeps it a table; empty for
/// any other verdict, as there is nothing to split for.
pub fn frontier(mir: &Mir, inputs: Option<&IncrementalInputs>) -> Frontier {
    if !matches!(classify(mir, inputs).verdict, Verdict::Keep(_)) {
        return Frontier::default();
    }
    let mut out = Frontier::default();
    let mut seen = BTreeSet::new();
    for id in mir.reachable() {
        let Some(RelOp::CteRef { cte, .. }) = mir.rels.get(id).map(|r| &r.op) else {
            continue;
        };
        if !seen.insert(*cte) {
            continue;
        }
        let Some(def) = mir.ctes.get(*cte) else {
            continue;
        };
        let name = def
            .name
            .map_or_else(|| "?".to_string(), |n| mir.syms.text(n).to_string());
        if reads_a_cte(mir, def.body, *cte) {
            out.refusals.push(Refusal::ReadsCte { cte: name });
            continue;
        }
        let analysis = classify_at(mir, def.body, inputs);
        if !analysis.incremental() {
            out.refusals.push(Refusal::NotIncremental { cte: name });
            continue;
        }
        match reduction(mir, def.body) {
            Some(reduces) => out.cuts.push(Cut {
                cte: name,
                body: def.body,
                analysis,
                reduces,
            }),
            None => out.refusals.push(Refusal::NoReduction { cte: Some(name) }),
        }
    }
    if out.cuts.is_empty() && out.refusals.is_empty() {
        // no CTE at all: the growing work shares a SELECT with the blocker
        out.refusals.push(match reduction(mir, mir.root) {
            Some(_) => Refusal::NoBoundary,
            None => Refusal::NoReduction { cte: None },
        });
    }
    out
}

fn reads_a_cte(mir: &Mir, body: RelId, own: CteId) -> bool {
    mir.reachable_from(&[body]).into_iter().any(|id| {
        matches!(mir.rels.get(id).map(|r| &r.op), Some(RelOp::CteRef { cte, .. }) if *cte != own)
    })
}

/// What in the part makes its output smaller than its input.
fn reduction(mir: &Mir, root: RelId) -> Option<&'static str> {
    let ops: Vec<&RelOp> = mir
        .reachable_from(&[root])
        .into_iter()
        .filter_map(|id| mir.rels.get(id).map(|r| &r.op))
        .collect();
    if ops.iter().any(|op| matches!(op, RelOp::Aggregate { .. })) {
        Some("aggregate")
    } else if ops.iter().any(|op| matches!(op, RelOp::Distinct(_))) {
        Some("distinct")
    } else if ops.iter().any(|op| matches!(op, RelOp::Filter { .. })) {
        Some("filter")
    } else {
        None
    }
}

/// Format 2: `(frontier MODEL (cut CTE (boundary …) (verdict …) …) (refused …))`.
pub fn frontier_sexp(mir: &Mir, model: &str, frontier: &Frontier) -> Sexp {
    let mut items = vec![atom(model)];
    for cut in &frontier.cuts {
        let mut parts = vec![atom(&cut.cte)];
        let mut boundary = vec![atom("cte")];
        if let Some(span) = mir.rels.get(cut.body).and_then(|r| mir.prov.span(r.prov)) {
            boundary.push(list(
                "span",
                [atom(span.start.to_string()), atom(span.end.to_string())],
            ));
        }
        parts.push(list("boundary", boundary));
        parts.push(verdict_sexp(mir, &cut.analysis.verdict));
        parts.push(list("reduces", [atom(cut.reduces)]));
        items.push(list("cut", parts));
    }
    items.extend(frontier.refusals.iter().map(refusal_sexp));
    list("frontier", items)
}

pub fn refusal_sexp(refusal: &Refusal) -> Sexp {
    let (key, cte, because) = match refusal {
        Refusal::NoReduction { cte } => (
            "no-reduction",
            cte.clone(),
            "materialized, this part is as large as the input it reads",
        ),
        Refusal::NoBoundary => (
            "no-boundary",
            None,
            "the growing work shares one SELECT with what blocks it; move it into a CTE",
        ),
        Refusal::ReadsCte { cte } => (
            "reads-cte",
            Some(cte.clone()),
            "it reads another CTE of the model, which a model of its own could not",
        ),
        Refusal::NotIncremental { cte } => (
            "not-incremental",
            Some(cte.clone()),
            "this part is not incremental on its own either",
        ),
    };
    let mut items = vec![atom(key)];
    if let Some(cte) = cte {
        items.push(list("cte", [atom(cte)]));
    }
    items.push(list("because", [string(because)]));
    list("refused", items)
}
