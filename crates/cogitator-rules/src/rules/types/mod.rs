//! `type.*` rules: values compared or aggregated across type families
//! the warehouse can only coerce implicitly. They report as diagnostics
//! outside the perf profile (`types.checks`, specs/ir-pipeline/08), at
//! Warning only when every type involved is proven ([`proven`]).

pub mod aggregate_text;
pub mod text_temporal_compare;

use cogitator_ir::mir::{Mir, RelId, ScalarId, ScalarKind, TableRef};
use cogitator_ir::SqlType;

use crate::cx::Cx;
use crate::findings::Evidence;
use crate::meta::Severity;
use crate::rules::join::support::scan_of;

/// The coarse families a comparison can mix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Family {
    Number,
    Text,
    Bool,
    Temporal,
}

impl Family {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Family::Number => "a number",
            Family::Text => "text",
            Family::Bool => "a boolean",
            Family::Temporal => "a date or time",
        }
    }
}

/// The family of a type, or `None` when it would prove nothing.
pub(crate) fn family(ty: &SqlType) -> Option<Family> {
    Some(match ty {
        SqlType::Int { .. } | SqlType::Decimal { .. } | SqlType::Float { .. } => Family::Number,
        SqlType::Text => Family::Text,
        SqlType::Bool => Family::Bool,
        SqlType::Date | SqlType::Time | SqlType::Timestamp { .. } => Family::Temporal,
        _ => return None,
    })
}

/// The type of a scalar as lowering stated it.
pub(crate) fn type_of(mir: &Mir, id: ScalarId) -> Option<&SqlType> {
    Some(&mir.scalars.get(id)?.ty.ty)
}

/// True when the scalar's type rests on evidence that holds: an
/// explicit cast, or a column of a model or source whose schema marks
/// the type proven (SQL inference over proven inputs, the catalog, an
/// enforced contract). `rel` is the relation the scalar reads from.
pub(crate) fn proven(mir: &Mir, cx: &Cx<'_>, rel: RelId, id: ScalarId) -> bool {
    match mir.scalars.get(id).map(|s| &s.kind) {
        Some(ScalarKind::Cast { .. }) => true,
        Some(ScalarKind::Col(col)) => {
            let (Some(schemas), Some((table, name))) = (cx.schemas, scan_of(mir, rel, *col)) else {
                return false;
            };
            let schema = match table {
                TableRef::Model { name, .. } => schemas.model(mir.syms.text(*name)),
                TableRef::Source { source, table } => {
                    schemas.source(mir.syms.text(*source), mir.syms.text(*table))
                }
                _ => None,
            };
            schema.is_some_and(|s| {
                s.columns
                    .iter()
                    .any(|c| c.proven_type && c.name.eq_ignore_ascii_case(mir.syms.text(name)))
            })
        }
        _ => false,
    }
}

/// Warning when every side is proven, else Info.
pub(crate) fn severity(all_proven: bool) -> Severity {
    if all_proven {
        Severity::Warning
    } else {
        Severity::Info
    }
}

/// Column types come from the catalog when one was read, else from SQL
/// inference and YAML.
pub(crate) fn evidence(cx: &Cx<'_>) -> Evidence {
    match cx.dag.and_then(|d| d.catalog_generated_at.clone()) {
        Some(generated_at) => Evidence::Catalog { generated_at },
        None => Evidence::Exact,
    }
}

/// Every scalar in the tree under `root`, root first.
pub(crate) fn subtree(mir: &Mir, root: ScalarId) -> Vec<ScalarId> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    let mut budget = mir.scalars.len() + 1;
    while let Some(id) = stack.pop() {
        if budget == 0 {
            break;
        }
        budget -= 1;
        let Some(node) = mir.scalars.get(id) else {
            continue;
        };
        out.push(id);
        if !matches!(node.kind, ScalarKind::Subquery { .. }) {
            stack.extend(node.kind.children());
        }
    }
    out
}
