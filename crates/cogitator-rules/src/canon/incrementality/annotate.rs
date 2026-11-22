//! The analysis as an S-expression (specs/ir-pipeline/10 §6, format 1):
//! the verdict, then the plan with each node's DBSP class appended —
//! `(scan events (class grows))`, `(window … (class blocked
//! window-over-whole-table))`.

use cogitator_ir::mir::{rel_sexp, Mir, RelId};
use cogitator_ir::sexp::{atom, list, Sexp};

use super::{Analysis, Class, DeltaInput, Note, Verdict};

pub fn analysis_sexp(mir: &Mir, root: RelId, model: &str, analysis: &Analysis) -> Sexp {
    let annotate = |id: RelId| -> Vec<Sexp> {
        analysis
            .classes
            .get(&id)
            .map(|class| vec![class_sexp(class)])
            .unwrap_or_default()
    };
    let mut items = vec![atom(model), verdict_sexp(mir, &analysis.verdict)];
    items.extend(analysis.notes.iter().map(note_sexp));
    items.push(list("plan", [rel_sexp(mir, root, &annotate)]));
    list("incrementality", items)
}

pub fn class_sexp(class: &Class) -> Sexp {
    match class {
        Class::Blocked(blocker) => list("class", [atom("blocked"), atom(blocker.key())]),
        other => list("class", [atom(other.key())]),
    }
}

pub fn verdict_sexp(mir: &Mir, verdict: &Verdict) -> Sexp {
    match verdict {
        Verdict::Append { inputs, out_column } => list(
            "verdict",
            [
                atom("append"),
                list("strategy", [atom("append")]),
                list("high-water", [atom(out_column)]),
            ]
            .into_iter()
            .chain(inputs.iter().map(delta_sexp)),
        ),
        Verdict::ByBucket {
            input,
            bucket,
            keys,
        } => list(
            "verdict",
            [
                atom("by-bucket"),
                list("strategy", [atom("delete+insert")]),
                list("unique-key", keys.iter().map(atom)),
                list("bucket", [atom(bucket)]),
                delta_sexp(input),
            ],
        ),
        Verdict::ByKey {
            input,
            keys,
            strategy,
            high_water,
        } => {
            let mut items = vec![
                atom("by-key"),
                list("strategy", [atom(strategy.dbt_name())]),
                list("unique-key", keys.iter().map(atom)),
            ];
            if let Some(h) = high_water {
                items.push(list("high-water", [atom(h)]));
            }
            items.push(delta_sexp(input));
            list("verdict", items)
        }
        Verdict::Keep(blocked) => list(
            "verdict",
            std::iter::once(atom("keep")).chain(blocked.iter().map(|b| {
                let mut items = vec![atom(b.blocker.key())];
                if let Some(span) = mir.prov.span(b.prov) {
                    items.push(list(
                        "span",
                        [atom(span.start.to_string()), atom(span.end.to_string())],
                    ));
                }
                list("blocker", items)
            })),
        ),
        Verdict::Static => list("verdict", [atom("static")]),
        Verdict::Unknown => list("verdict", [atom("unknown")]),
    }
}

pub fn delta_sexp(input: &DeltaInput) -> Sexp {
    let how = if input.declared {
        "declared"
    } else {
        "inferred"
    };
    list(
        "delta",
        [list(
            "input",
            [atom(&input.relation), atom(&input.column), atom(how)],
        )],
    )
}

fn note_sexp(note: &Note) -> Sexp {
    match note {
        Note::Bilinear { other } => list("note", [atom("bilinear"), atom(other)]),
        Note::AssumesStatic(name) => list("note", [atom("assumes-static"), atom(name)]),
        Note::Having => list("note", [atom("having")]),
        Note::FloatSum => list("note", [atom("float-sum")]),
        Note::MinMax => list("note", [atom("min-max")]),
        Note::Holistic(func) => list("note", [atom("holistic"), atom(func)]),
    }
}
