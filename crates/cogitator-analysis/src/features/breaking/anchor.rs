//! Where a breaking-change diagnostic sits in the model's SQL, from the
//! HIR of the live template (`lineage::template_input`):
//!
//! - a retyped column: the select item that produces it — the root's, or
//!   when the root passes it on by `*`, the CTE's that computes it;
//! - a removed column whose name an added one looks like (a likely
//!   rename): the item of the added column, which took its place;
//! - any other removed column: its item is gone, so the root query's
//!   `select` keyword — the select list it is missing from.
//!
//! Every anchor is the item's exact text span, never a guess; when the
//! HIR cannot be built (Jinja control blocks, a parse hole) there is no
//! anchor and the diagnostic stays at the top of the file, as before.
//!
//! The same walk finds what "Keep '<old>' as an alias" re-adds: the
//! added column's expression, when its span is exact (a bare column or
//! literal), and where its select item ends.

use cogitator_dialects::Dialect;
use cogitator_ir::hir::{Body, ExprKind, Hir, QueryId, SelectItem};
use cogitator_ir::Span;
use cogitator_lsp::Range;
use cogitator_parser::{LineTable, ParsedDocument};
use cogitator_sql_front::{build_hir, SchemaEnv};
use serde::Serialize;

use crate::features::plan::diagnostics::span_range;
use crate::lineage::{has_control_block, template_input};

/// An output column's item in the model's SQL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemAnchor {
    /// The select item.
    pub range: Range,
    /// The expression's own text when its span is exact (a column or
    /// literal): what a "keep the old name" alias can repeat.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expr: Option<String>,
}

/// The model's anchors: one lookup per output column, plus the root's
/// `select` keyword.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Anchors {
    items: Vec<(String, ItemAnchor)>,
    select: Option<Range>,
}

impl Anchors {
    pub fn of(doc: &ParsedDocument, dialect: &Dialect) -> Self {
        if has_control_block(doc) {
            return Anchors::default();
        }
        let lowered = build_hir(&template_input(doc, dialect), &SchemaEnv::default());
        let Some(hir) = lowered.value.as_ref() else {
            return Anchors::default();
        };
        let table = LineTable::new(&doc.text);
        collect(hir, &doc.text, &table)
    }

    /// The item that produces `column` now.
    pub fn item(&self, column: &str) -> Option<&ItemAnchor> {
        self.items
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(column))
            .map(|(_, a)| a)
    }

    /// The root query's `select` keyword.
    pub fn select(&self) -> Option<Range> {
        self.select
    }
}

fn collect(hir: &Hir, text: &str, table: &LineTable) -> Anchors {
    let mut out = Anchors::default();
    let Some(root) = hir.root else {
        return out;
    };
    let resolve = |sym| hir.syms.resolve(sym).map(str::to_string);
    // one select item: its output name, its span (the expression to the
    // alias) and, for a bare column or literal, the expression's text
    let item = |item: &SelectItem| -> Option<(String, ItemAnchor)> {
        let SelectItem::Expr { expr, alias } = item else {
            return None; // a star's columns have no item of their own
        };
        let e = hir.exprs.get(*expr)?;
        let name = match (alias, &e.kind) {
            (Some(alias), _) => resolve(alias.folded)?,
            (None, ExprKind::Col(c)) => resolve(hir.cols.get(*c)?.name.folded)?,
            _ => return None,
        };
        let end = alias.map_or(e.span.end, |a| a.span.end);
        let range = span_range(text, table, Span::new(e.span.start, end))?;
        let expr = e
            .kind
            .span_is_exact()
            .then(|| text.get(e.span.start as usize..e.span.end as usize))
            .flatten()
            .map(str::to_string);
        Some((name, ItemAnchor { range, expr }))
    };
    // the root's items win; a name it passes on by star takes the
    // innermost-defined (last allocated) query's item of that name
    let mut queries: Vec<QueryId> = vec![root];
    let mut rest: Vec<QueryId> = hir
        .queries
        .iter()
        .map(|(id, _)| id)
        .filter(|id| *id != root)
        .collect();
    rest.reverse();
    queries.extend(rest);
    for id in queries {
        let Some(Body::Select(select)) = hir.queries.get(id).map(|q| &q.body) else {
            continue;
        };
        for (name, anchor) in select.items.iter().filter_map(item) {
            if out.item(&name).is_none() {
                out.items.push((name, anchor));
            }
        }
    }
    out.select = root_select(hir, root).and_then(|span_start| {
        let keyword = text.get(span_start as usize..span_start as usize + 6)?;
        keyword
            .eq_ignore_ascii_case("select")
            .then(|| span_range(text, table, Span::new(span_start, span_start + 6)))?
    });
    out
}

/// Where the root's first SELECT starts (the left of a set operation).
fn root_select(hir: &Hir, mut id: QueryId) -> Option<u32> {
    loop {
        match &hir.queries.get(id)?.body {
            Body::Select(select) => return Some(select.span.start),
            Body::SetOp { l, .. } => id = *l,
            Body::Values(_) | Body::Hole(_) => return None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cogitator_parser::{DocumentParser, JinjaSqlParser};

    fn anchors(sql: &str) -> Anchors {
        let doc = JinjaSqlParser.parse(sql, &Dialect::default()).unwrap();
        Anchors::of(&doc, &Dialect::default())
    }

    fn text_at(sql: &str, r: Range) -> String {
        let line = sql.lines().nth(r.start.line as usize).unwrap();
        line[r.start.character as usize..r.end.character as usize].to_string()
    }

    #[test]
    fn items_and_the_select_keyword() {
        let sql = "select\n    id as customer_key,\n    upper(name) as name\nfrom {{ ref('x') }}";
        let a = anchors(sql);
        let key = a.item("CUSTOMER_KEY").expect("the item");
        assert_eq!(text_at(sql, key.range), "id as customer_key");
        assert_eq!(key.expr.as_deref(), Some("id"));
        let name = a.item("name").expect("the item");
        assert_eq!(name.expr, None, "a call's span is not exact");
        assert_eq!(text_at(sql, a.select().unwrap()), "select");
        assert!(a.item("gone").is_none());
    }

    #[test]
    fn through_a_star_the_cte_item() {
        let sql = "with r as (\n  select id as k from {{ ref('x') }}\n)\nselect * from r";
        let a = anchors(sql);
        assert_eq!(text_at(sql, a.item("k").unwrap().range), "id as k");
        let select = a.select().unwrap();
        assert_eq!((select.start.line, select.start.character), (3, 0));
    }

    #[test]
    fn control_blocks_give_no_anchor() {
        let a = anchors("{% if x %}select 1 as a{% endif %}");
        assert_eq!(a, Anchors::default());
    }
}
