//! The rewrite `incr.could_be_incremental` offers: the model made an
//! incremental one, by two edits of the template (specs/ir-pipeline/10).
//!
//! 1. `config()`: `materialized='incremental'` with the strategy (and
//!    `unique_key` for buckets), editing the call there is or adding one;
//! 2. the delta filter on the one SELECT that reads the driving input,
//!    guarded by `is_incremental()` so a full refresh still builds all:
//!    - append: `col > max(out)` of `{{ this }}` — strictly after, as
//!      append adds what it is given and a row at the boundary would be
//!      appended twice;
//!    - buckets: `col >= max(bucket)` of `{{ this }}` — the newest bucket
//!      is recomputed whole and replaced by delete+insert;
//!    - one group key with a high-water column: `key in (select key from
//!      <input> where col > max(high_water))` — every group a new row
//!      touches is recomputed whole and replaced on the key;
//!    - either way `max(..) is null or …`: a model built empty has no
//!      newest value, and a comparison with NULL would let no row in, ever.
//!
//! testdata/oracle/incr_append_linear and incr_bucket_daily hold these
//! increments to full rebuilds (specs/ir-pipeline/10).
//!
//! An existing WHERE is parenthesized first, so an OR in it keeps its
//! meaning.
//!
//! Offered only where both places are certain: a template-origin plan,
//! exactly one SELECT reading the driving input, a qualifier for the
//! column wherever that SELECT reads more than one relation. Otherwise the
//! finding stands alone and says what to write. It changes what the
//! model is, so it is never preferred and never part of fix-all.

use cogitator_ir::hir::{Body, FromId, FromKind, Hir, JinjaKind, RelTarget, Select};
use cogitator_ir::origin::SourceOrigin;
use cogitator_ir::Span;

use crate::canon::incrementality::{DeltaInput, Verdict};
use crate::findings::{Fix, FixEdit};
use crate::meta::FixSafety;
use crate::rules::join::support::from_leaves;

pub(super) fn rewrite(hir: &Hir, text: &str, verdict: &Verdict) -> Option<Fix> {
    if !matches!(hir.origin, SourceOrigin::Template { .. }) {
        return None;
    }
    let quoted = |keys: &[String]| match keys {
        [one] => format!("'{one}'"),
        many => format!(
            "[{}]",
            many.iter()
                .map(|k| format!("'{k}'"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    };
    // the input, the config arguments, and the newest value in the model
    let (input, args, newest) = match verdict {
        Verdict::Append { inputs, out_column } => {
            let [input] = inputs.as_slice() else {
                return None;
            };
            (
                input,
                "incremental_strategy='append'".to_string(),
                out_column,
            )
        }
        Verdict::ByBucket {
            input,
            bucket,
            keys,
        } => (
            input,
            format!(
                "incremental_strategy='delete+insert', unique_key={}",
                quoted(keys)
            ),
            bucket,
        ),
        // the touched groups: one key, and a column with each group's
        // newest event time to find the new rows by
        Verdict::ByKey {
            input,
            keys,
            strategy,
            high_water: Some(high_water),
        } if keys.len() == 1 => (
            input,
            format!(
                "incremental_strategy='{}', unique_key={}",
                strategy.dbt_name(),
                quoted(keys)
            ),
            high_water,
        ),
        _ => return None,
    };
    let (select, qualifier, leaf) = driving_select(hir, input)?;
    let qualify = |name: &str| match &qualifier {
        Some(q) => format!("{q}.{name}"),
        None => name.to_string(),
    };
    let max = format!("(select max({newest}) from {{{{ this }}}})");
    let column = qualify(&input.column);
    let grows = match verdict {
        Verdict::Append { .. } => format!("{column} > {max}"),
        Verdict::ByBucket { .. } => format!("{column} >= {max}"),
        Verdict::ByKey { keys, .. } => {
            // every row of a group a new row touches, so the group is
            // recomputed whole
            let key = &keys[0];
            let from = hir.from.get(leaf)?;
            if !from
                .output
                .iter()
                .any(|c| hir.syms.text(c.name).eq_ignore_ascii_case(key))
            {
                return None; // the key is renamed on its way out
            }
            let source = text
                .get(from.span.start as usize..from.span.end as usize)?
                .trim();
            format!(
                "{} in (select {key} from {source} where {} > {max})",
                qualify(key),
                input.column
            )
        }
        _ => return None,
    };
    let mut edits = config_edits(hir, text, &args)?;
    let guard = |keyword: &str| {
        format!(
            "\n{{% if is_incremental() %}}\n{keyword} ({max} is null or {grows})\n{{% endif %}}"
        )
    };
    match select.filter.and_then(|f| hir.exprs.get(f)).map(|e| e.span) {
        Some(filter) => {
            edits.push(FixEdit {
                span: Span {
                    start: filter.start,
                    end: filter.start,
                },
                new_text: "(".into(),
            });
            edits.push(FixEdit {
                span: Span {
                    start: filter.end,
                    end: filter.end,
                },
                new_text: format!("){}", guard("and")),
            });
        }
        None => {
            let end = select.from.iter().filter_map(|f| from_end(hir, *f)).max()?;
            edits.push(FixEdit {
                span: Span { start: end, end },
                new_text: guard("where"),
            });
        }
    }
    edits.sort_by_key(|e| (e.span.start, e.span.end));
    Some(Fix {
        title: format!(
            "Make it an incremental model on `{}.{}`",
            input.relation, input.column
        ),
        safety: FixSafety::ChangesIntent,
        edits,
    })
}

/// Where a FROM item ends in the text: its own span stops before an
/// alias, so the aliases of every relation in it count too.
fn from_end(hir: &Hir, id: FromId) -> Option<u32> {
    let own = hir.from.get(id)?.span.end;
    let aliases = from_leaves(hir, id).into_iter().filter_map(|leaf| {
        match hir.from.get(leaf).map(|f| &f.kind) {
            Some(
                FromKind::Base { alias: Some(a), .. } | FromKind::Derived { alias: Some(a), .. },
            ) => Some(a.span.end),
            _ => None,
        }
    });
    aliases.chain(std::iter::once(own)).max()
}

/// The one SELECT reading `input`, and what to qualify its column with:
/// None when the SELECT reads that relation alone.
fn driving_select<'h>(
    hir: &'h Hir,
    input: &DeltaInput,
) -> Option<(&'h Select, Option<String>, FromId)> {
    let mut found = Vec::new();
    for query in hir.queries.values() {
        let Body::Select(select) = &query.body else {
            continue;
        };
        let leaves: Vec<_> = select
            .from
            .iter()
            .flat_map(|f| from_leaves(hir, *f))
            .collect();
        for leaf in &leaves {
            let Some(FromKind::Base { target, alias }) = hir.from.get(*leaf).map(|f| &f.kind)
            else {
                continue;
            };
            if !reads(hir, target, &input.relation) {
                continue;
            }
            let qualifier = match (alias, leaves.len()) {
                (Some(alias), _) => Some(hir.syms.text(alias.folded).to_string()),
                (None, 1) => None,
                (None, _) => return None, // no name to qualify the column with
            };
            found.push((&**select, qualifier, *leaf));
        }
    }
    match found.len() {
        1 => found.pop(),
        _ => None,
    }
}

fn reads(hir: &Hir, target: &RelTarget, relation: &str) -> bool {
    match target {
        RelTarget::Model { name, .. } => hir.syms.text(*name) == relation,
        RelTarget::Source { source, table } => {
            format!("{}.{}", hir.syms.text(*source), hir.syms.text(*table)) == relation
        }
        _ => false,
    }
}

/// `materialized='incremental'` and `args`, in the `config()` call there
/// is, or in one added at the top.
fn config_edits(hir: &Hir, text: &str, args: &str) -> Option<Vec<FixEdit>> {
    let configs: Vec<Span> = hir
        .jinja
        .values()
        .filter(|node| node.kind == JinjaKind::Config)
        .map(|node| node.span)
        .collect();
    let [span] = configs.as_slice() else {
        if configs.is_empty() {
            return Some(vec![FixEdit {
                span: Span { start: 0, end: 0 },
                new_text: format!("{{{{ config(materialized='incremental', {args}) }}}}\n"),
            }]);
        }
        return None;
    };
    let call = text.get(span.start as usize..span.end as usize)?;
    let close = call.rfind(')')?;
    let mut edits = Vec::new();
    match materialized_value(call) {
        Some((start, end)) => {
            edits.push(FixEdit {
                span: Span {
                    start: span.start + start as u32,
                    end: span.start + end as u32,
                },
                new_text: "'incremental'".into(),
            });
            edits.push(FixEdit {
                span: Span {
                    start: span.start + close as u32,
                    end: span.start + close as u32,
                },
                new_text: format!(", {args}"),
            });
        }
        None => {
            let empty = call[call.find('(')? + 1..close].trim().is_empty();
            let sep = if empty { "" } else { ", " };
            edits.push(FixEdit {
                span: Span {
                    start: span.start + close as u32,
                    end: span.start + close as u32,
                },
                new_text: format!("{sep}materialized='incremental', {args}"),
            });
        }
    }
    Some(edits)
}

/// The quoted value of `materialized=` in a `config(...)` call, as byte
/// offsets into `call`, quotes included.
fn materialized_value(call: &str) -> Option<(usize, usize)> {
    let key = call.find("materialized")?;
    let rest = &call[key + "materialized".len()..];
    let eq = rest.find('=')?;
    if !rest[..eq].trim().is_empty() {
        return None;
    }
    let after = &rest[eq + 1..];
    let lead = after.len() - after.trim_start().len();
    let quote = after[lead..]
        .chars()
        .next()
        .filter(|c| *c == '\'' || *c == '"')?;
    let body = &after[lead + 1..];
    let close = body.find(quote)?;
    let start = key + "materialized".len() + eq + 1 + lead;
    Some((start, start + close + 2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_materialized_value_is_found_with_its_quotes() {
        let call = "{{ config(materialized = \"table\", tags=['x']) }}";
        let (s, e) = materialized_value(call).unwrap();
        assert_eq!(&call[s..e], "\"table\"");
        assert_eq!(materialized_value("{{ config(tags=['x']) }}"), None);
    }
}
