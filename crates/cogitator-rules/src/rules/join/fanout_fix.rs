//! "Deduplicate <relation> on <key>" for `join.fanout`: a CTE keeping
//! the first row per key (QUALIFY where the dialect has it, DISTINCT ON
//! on Postgres, a filtered subquery elsewhere) and the joined relation
//! replaced by it. It changes results by design — which duplicate stays
//! is the `order by`, the key itself until someone picks better — so it
//! is never preferred. Anything not as expected withholds the fix.

use cogitator_ir::common::JoinKind;
use cogitator_ir::hir::{FromId, FromKind, Hir, RelTarget};
use cogitator_ir::lir::Backend;
use cogitator_ir::origin::SourceOrigin;
use cogitator_ir::Span;

use super::text::slice;
use crate::findings::{Fix, FixEdit};
use crate::meta::FixSafety;

pub(super) fn dedup(
    hir: &Hir,
    text: &str,
    join: FromId,
    kind: JoinKind,
    keys: &[String],
    dialect: Option<Backend>,
) -> Option<Fix> {
    if !matches!(hir.origin, SourceOrigin::Template { .. }) || keys.is_empty() {
        return None;
    }
    let FromKind::Join { l, r, .. } = &hir.from.get(join)?.kind else {
        return None;
    };
    let side = if kind == JoinKind::Right { *l } else { *r };
    let item = hir.from.get(side)?;
    let FromKind::Base { target, alias } = &item.kind else {
        return None;
    };
    let whole = slice(text, item.span)?;
    // the item's span may or may not reach over its alias
    let (relation_text, replacement) = match alias {
        Some(a) if a.span.start < item.span.end => {
            let before = text.get(item.span.start as usize..a.span.start as usize)?;
            let alias_text = slice(text, a.span)?;
            (strip_as(before).to_string(), Some(alias_text.to_string()))
        }
        Some(_) => (strip_as(whole).to_string(), None),
        None => (whole.to_string(), Some(base_name(hir, target)?)),
    };
    if relation_text.is_empty() {
        return None;
    }
    let base = base_name(hir, target)?;
    let name = format!("{base}_dedup");
    let body = body(&relation_text, keys, dialect);
    let insert = insertion(hir, text, target, &name, &body)?;
    Some(Fix {
        title: format!("Deduplicate {base} on {}", keys.join(", ")),
        safety: FixSafety::ChangesIntent,
        edits: vec![
            insert,
            FixEdit {
                span: item.span,
                new_text: match replacement {
                    Some(alias) => format!("{name} as {alias}"),
                    None => name.clone(),
                },
            },
        ],
    })
}

fn strip_as(before: &str) -> &str {
    let trimmed = before.trim_end();
    let lower = trimmed.to_ascii_lowercase();
    if lower.ends_with(" as") {
        trimmed[..trimmed.len() - 3].trim_end()
    } else {
        trimmed
    }
}

/// The name the CTE is called after: the model, the source table, or
/// the CTE the join reads.
fn base_name(hir: &Hir, target: &RelTarget) -> Option<String> {
    Some(match target {
        RelTarget::Model { name, .. } => hir.syms.text(*name).to_string(),
        RelTarget::Source { table, .. } => hir.syms.text(*table).to_string(),
        RelTarget::Cte(id) => hir.syms.text(hir.ctes.get(*id)?.name.folded).to_string(),
        _ => return None,
    })
}

fn body(relation: &str, keys: &[String], dialect: Option<Backend>) -> String {
    let keys = keys.join(", ");
    match dialect {
        Some(Backend::Snowflake | Backend::BigQuery | Backend::DuckDb) => format!(
            "select * from {relation}\n    qualify row_number() over (partition by {keys} order by {keys}) = 1"
        ),
        Some(Backend::Postgres) => {
            format!("select distinct on ({keys}) * from {relation}\n    order by {keys}")
        }
        _ => format!(
            "select * from (\n        select *, row_number() over (partition by {keys} order by {keys}) as dedup_row\n        from {relation}\n    ) as deduplicated\n    where dedup_row = 1"
        ),
    }
}

/// Where the new CTE goes: right after the CTE it reads, else first in
/// the root query's WITH, else a new WITH before the root SELECT.
fn insertion(hir: &Hir, text: &str, target: &RelTarget, name: &str, body: &str) -> Option<FixEdit> {
    let cte = format!("{name} as (\n    {body}\n)");
    if let RelTarget::Cte(id) = target {
        let def = hir.ctes.get(*id)?;
        if !slice(text, def.span)?.ends_with(')') {
            return None;
        }
        return Some(FixEdit {
            span: Span::new(def.span.end, def.span.end),
            new_text: format!(",\n{cte}"),
        });
    }
    let root = hir.queries.get(hir.root?)?;
    if let Some(first) = root.ctes.first().and_then(|c| hir.ctes.get(*c)) {
        let at = first.span.start;
        let starts = text
            .get(at as usize..)?
            .to_ascii_lowercase()
            .starts_with(hir.syms.text(first.name.folded));
        if !starts {
            return None;
        }
        return Some(FixEdit {
            span: Span::new(at, at),
            new_text: format!("{cte},\n"),
        });
    }
    let at = root.span.start;
    if !text
        .get(at as usize..)?
        .to_ascii_lowercase()
        .starts_with("select")
    {
        return None;
    }
    Some(FixEdit {
        span: Span::new(at, at),
        new_text: format!("with {cte}\n"),
    })
}
