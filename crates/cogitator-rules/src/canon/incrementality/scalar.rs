//! What incrementality needs to know of a scalar: whether it is a
//! function of its row alone (deterministic), whether it grows with a
//! column that only grows (monotone), and which subqueries it reads.

use std::collections::BTreeMap;

use cogitator_ir::common::{BinaryOp, Lit};
use cogitator_ir::mir::{ColId, Mir, RelId, ScalarId, ScalarKind};
use cogitator_ir::types::SqlType;

/// Functions whose value is not a function of the row: the clock, the
/// session, randomness. Lower case; dialect spellings included.
const NON_DETERMINISTIC: &[&str] = &[
    "current_timestamp",
    "current_date",
    "current_time",
    "localtimestamp",
    "localtime",
    "now",
    "getdate",
    "sysdate",
    "systimestamp",
    "current_datetime",
    "random",
    "rand",
    "uuid",
    "gen_random_uuid",
    "uuid_string",
    "generate_uuid",
    "newid",
    "current_user",
    "current_role",
    "session_user",
];

/// Functions monotone non-decreasing in their temporal argument: a
/// bucket of a growing time grows too.
const MONOTONE: &[&str] = &[
    "date_trunc",
    "trunc",
    "date",
    "to_date",
    "timestamp_trunc",
    "datetime_trunc",
    "date_bin",
    "time_bucket",
    "day",
    "to_timestamp",
    "timestamp",
    "datetime",
];

/// What a scalar tree holds that matters here.
#[derive(Debug, Default)]
pub(super) struct ScalarFacts {
    /// The first non-deterministic function, by name.
    pub nondeterministic: Option<String>,
    /// Subqueries, each relation once.
    pub subqueries: Vec<RelId>,
    /// An `Opaque` scalar: something the front end could not read.
    pub opaque: bool,
}

pub(super) fn facts(mir: &Mir, root: ScalarId) -> ScalarFacts {
    let mut out = ScalarFacts::default();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let Some(scalar) = mir.scalars.get(id) else {
            out.opaque = true;
            continue;
        };
        match &scalar.kind {
            ScalarKind::Call { name, .. } => {
                let name = mir.syms.text(*name).to_ascii_lowercase();
                if out.nondeterministic.is_none() && NON_DETERMINISTIC.contains(&name.as_str()) {
                    out.nondeterministic = Some(name);
                }
            }
            ScalarKind::Subquery { rel, .. } => {
                if !out.subqueries.contains(rel) {
                    out.subqueries.push(*rel);
                }
            }
            ScalarKind::Opaque { .. } => out.opaque = true,
            _ => {}
        }
        stack.extend(scalar.kind.children());
    }
    out
}

/// The monotone input column `root` grows with: the column itself, a
/// cast of it to a temporal type, a bucket of it (`date_trunc('day', ts)`)
/// or it shifted by a constant (`ts + interval '1 hour'`). `mono` maps a
/// growing column to the driving input it comes from.
pub(super) fn monotone_of(
    mir: &Mir,
    root: ScalarId,
    mono: &BTreeMap<ColId, usize>,
) -> Option<ColId> {
    let scalar = mir.scalars.get(root)?;
    match &scalar.kind {
        ScalarKind::Col(c) => mono.contains_key(c).then_some(*c),
        ScalarKind::Cast { e, ty } if temporal(ty) => monotone_of(mir, *e, mono),
        ScalarKind::Call {
            name,
            args,
            distinct: false,
        } => {
            let name = mir.syms.text(*name).to_ascii_lowercase();
            if !MONOTONE.contains(&name.as_str()) {
                return None;
            }
            // exactly one argument grows; the rest are constants
            let mut growing = None;
            for arg in args {
                match monotone_of(mir, *arg, mono) {
                    Some(col) if growing.is_none() => growing = Some(col),
                    Some(_) => return None,
                    None if is_constant(mir, *arg) => {}
                    None => return None,
                }
            }
            growing
        }
        ScalarKind::Binary {
            op: BinaryOp::Plus | BinaryOp::Minus,
            l,
            r,
        } if is_constant(mir, *r) => monotone_of(mir, *l, mono),
        ScalarKind::Binary {
            op: BinaryOp::Plus,
            l,
            r,
        } if is_constant(mir, *l) => monotone_of(mir, *r, mono),
        _ => None,
    }
}

fn is_constant(mir: &Mir, id: ScalarId) -> bool {
    matches!(
        mir.scalars.get(id).map(|s| &s.kind),
        Some(ScalarKind::Lit(
            Lit::Number(_) | Lit::Str(_) | Lit::Interval(_) | Lit::Typed { .. }
        ))
    )
}

pub(super) fn temporal(ty: &SqlType) -> bool {
    matches!(ty, SqlType::Date | SqlType::Timestamp { .. })
}

/// Is the column a column only an append-only table's new rows carry the
/// newest values of, by its name? Updated-at columns are left out: their
/// rows change.
pub(super) fn looks_like_event_time(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    if name.starts_with("updated") || name.starts_with("modified") || name.contains("_updated") {
        return false;
    }
    const EXACT: &[&str] = &[
        "ts",
        "event_time",
        "event_ts",
        "event_timestamp",
        "timestamp",
        "loaded_at",
        "_loaded_at",
        "inserted_at",
        "ingested_at",
        "created_at",
        "created",
        "occurred_at",
    ];
    EXACT.contains(&name.as_str())
        || name.ends_with("_at")
        || name.ends_with("_ts")
        || name.ends_with("_time")
        || name.starts_with("created")
}

/// Aggregates whose value is not maintained from its old value and the
/// new rows alone — exact here only because whole groups are recomputed.
pub(super) fn holistic(func: &str, distinct: bool) -> bool {
    distinct
        || matches!(
            func,
            "median"
                | "percentile_cont"
                | "percentile_disc"
                | "quantile"
                | "mode"
                | "listagg"
                | "string_agg"
                | "array_agg"
                | "group_concat"
                | "approx_count_distinct"
        )
}
