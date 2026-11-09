//! The output steps of a SELECT: Project (or `Opaque` for an open list),
//! DISTINCT and ORDER BY. A key the projection drops is carried as a
//! hidden item and trimmed above the Sort.

use std::collections::BTreeSet;

use cogitator_ir::common::{HoleKind, Lit};
use cogitator_ir::hir::{
    Distinct, ExprId, ExprKind, JinjaId, OrderItem, Query, Resolution, Select,
};
use cogitator_ir::mir::{ColId, HirNode, ProvId, RelId, RelOp, ScalarId, ScalarKind, SortKey};
use cogitator_ir::{ColType, DegradationKind, Span, Symbol};

use super::equal::same_expr;
use super::query::QueryOut;
use super::star::{Expanded, Field, Item};
use super::{Ctx, OpaqueSpec};

/// Where one ORDER BY key reads from.
enum SortSource {
    Output(usize),
    Hidden(ScalarId),
}

struct Planned {
    source: SortSource,
    desc: Option<bool>,
    nulls_first: Option<bool>,
}

impl Ctx<'_> {
    pub(super) fn finish_select(
        &mut self,
        query: &Query,
        select: &Select,
        expanded: Expanded,
        rel: RelId,
        demand: &[Symbol],
        prov: ProvId,
    ) -> QueryOut {
        if expanded.open {
            let out = self.open_output(rel, &expanded, demand, select.span, prov);
            return self.order_outputs(query, out, prov);
        }
        let distinct = select.distinct != Distinct::None;
        let plans: Vec<Planned> = query
            .order_by
            .iter()
            .filter_map(|key| self.plan_key(key, &expanded, distinct))
            .collect();
        let mut used = BTreeSet::new();
        let mut items: Vec<(ColId, ScalarId)> = Vec::with_capacity(expanded.items.len());
        for item in &expanded.items {
            let (scalar, name) = self.item_scalar(item, prov);
            let col = self.output_col(scalar, name, &mut used);
            items.push((col, scalar));
        }
        let visible = items.len();
        let mut hidden = Vec::new();
        for plan in &plans {
            if let SortSource::Hidden(scalar) = plan.source {
                let col = self.output_col(scalar, None, &mut used);
                items.push((col, scalar));
                hidden.push(col);
            }
        }
        let cols: Vec<ColId> = items.iter().map(|(c, _)| *c).collect();
        let project = self.mir.add_rel(RelOp::Project { input: rel, items }, prov);
        // DISTINCT ON replaces the output columns, so keys are built after it
        let (mut rel, outputs) = self.apply_distinct(project, select, &cols[..visible], prov);
        let mut hidden = hidden.into_iter();
        let keys: Vec<SortKey> = plans
            .iter()
            .filter_map(|plan| {
                let col = match plan.source {
                    SortSource::Output(i) => outputs.get(i).copied()?,
                    SortSource::Hidden(_) => hidden.next()?,
                };
                let ty = self.col_type(col);
                Some(SortKey {
                    expr: self.col_scalar(col, ty, prov),
                    desc: plan.desc,
                    nulls_first: plan.nulls_first,
                })
            })
            .collect();
        if !keys.is_empty() {
            rel = self.mir.add_rel(RelOp::Sort { input: rel, keys }, prov);
        }
        if cols.len() > visible {
            rel = self.trim(rel, &outputs, prov);
        }
        QueryOut {
            rel,
            cols: outputs,
            open: false,
        }
    }

    fn item_scalar(&mut self, item: &Item, prov: ProvId) -> (ScalarId, Option<Symbol>) {
        match item {
            Item::Expr { expr, name } => (self.expr(*expr), *name),
            Item::Field(field) => (self.field_scalar(field, prov), field.name),
            Item::Jinja { jinja, span } => (self.jinja_scalar(*jinja, *span), None),
        }
    }

    fn field_scalar(&mut self, field: &Field, prov: ProvId) -> ScalarId {
        if self.env.grouped && !self.env.key_cols.contains(&field.col) {
            self.degrade(
                DegradationKind::OpaqueExpression,
                Span::default(),
                "a starred column is neither grouped nor aggregated",
            );
            return self.opaque_scalar(Vec::new(), HoleKind::Unsupported, Span::default(), prov);
        }
        let left = self.col_scalar(field.col, field.ty.clone(), prov);
        let Some((right_col, right_ty)) = &field.merged else {
            return left;
        };
        let right = self.col_scalar(*right_col, right_ty.clone(), prov);
        let name = self.mir.syms.intern("coalesce");
        self.mir.add_scalar(
            ScalarKind::Call {
                name,
                args: vec![left, right],
                distinct: false,
            },
            field.ty.clone(),
            prov,
        )
    }

    fn jinja_scalar(&mut self, jinja: JinjaId, span: Span) -> ScalarId {
        let prov = self.prov(HirNode::Jinja(jinja), span);
        self.degrade(
            DegradationKind::OpaqueExpression,
            span,
            "a Jinja expression has no static value",
        );
        self.opaque_scalar(Vec::new(), HoleKind::Jinja, span, prov)
    }

    /// Reuses the input's `ColId` for a true pass-through; a rename, a
    /// repeat or a nullability change gets a fresh column.
    fn output_col(
        &mut self,
        scalar: ScalarId,
        name: Option<Symbol>,
        used: &mut BTreeSet<ColId>,
    ) -> ColId {
        let ty = self.scalar_type(scalar);
        if let Some(ScalarKind::Col(col)) = self.mir.scalars.get(scalar).map(|s| &s.kind) {
            let col = *col;
            let def = self.col_type(col);
            let same_name = name.is_none() || name == self.col_name(col);
            if same_name && def == ty && used.insert(col) {
                return col;
            }
        }
        self.mir.add_col(name, ty)
    }

    fn open_output(
        &mut self,
        input: RelId,
        expanded: &Expanded,
        demand: &[Symbol],
        span: Span,
        prov: ProvId,
    ) -> QueryOut {
        let mut cols: Vec<(Option<Symbol>, ColType)> = expanded
            .items
            .iter()
            .map(|item| match item {
                Item::Expr { name, .. } => (*name, ColType::UNKNOWN),
                Item::Field(field) => (field.name, field.ty.clone()),
                Item::Jinja { .. } => (None, ColType::UNKNOWN),
            })
            .collect();
        for name in demand {
            if !cols.iter().any(|(n, _)| *n == Some(*name)) {
                cols.push((Some(*name), ColType::UNKNOWN));
            }
        }
        let spec = OpaqueSpec {
            cols,
            open: true,
            kind: HoleKind::Unsupported,
            span,
        };
        let (rel, cols) = self.opaque_rel(vec![input], spec, prov);
        QueryOut {
            rel,
            cols,
            open: true,
        }
    }

    fn apply_distinct(
        &mut self,
        rel: RelId,
        select: &Select,
        cols: &[ColId],
        prov: ProvId,
    ) -> (RelId, Vec<ColId>) {
        match &select.distinct {
            Distinct::None => (rel, cols.to_vec()),
            Distinct::All => {
                // LIR's key-only Aggregate names these key scalars
                for col in cols {
                    let ty = self.col_type(*col);
                    self.col_scalar(*col, ty, prov);
                }
                (self.mir.add_rel(RelOp::Distinct(rel), prov), cols.to_vec())
            }
            Distinct::On(_) => {
                self.degrade(
                    DegradationKind::UnsupportedSyntax,
                    select.span,
                    "DISTINCT ON has no MIR form",
                );
                let spec = OpaqueSpec {
                    cols: cols
                        .iter()
                        .map(|c| (self.col_name(*c), self.col_type(*c)))
                        .collect(),
                    open: false,
                    kind: HoleKind::Unsupported,
                    span: select.span,
                };
                self.opaque_rel(vec![rel], spec, prov)
            }
        }
    }

    /// A Project passing `cols` through under their own ids.
    fn trim(&mut self, input: RelId, cols: &[ColId], prov: ProvId) -> RelId {
        let items = cols
            .iter()
            .map(|c| {
                let ty = self.col_type(*c);
                (*c, self.col_scalar(*c, ty, prov))
            })
            .collect();
        self.mir.add_rel(RelOp::Project { input, items }, prov)
    }

    fn plan_key(
        &mut self,
        key: &OrderItem,
        expanded: &Expanded,
        distinct: bool,
    ) -> Option<Planned> {
        let hir = self.hir;
        let span = hir.exprs.get(key.expr).map_or(Span::default(), |e| e.span);
        let source = match self.output_of(key.expr, expanded) {
            Some(i) => SortSource::Output(i),
            None if distinct => {
                self.degrade(
                    DegradationKind::UnsupportedSyntax,
                    span,
                    "this ORDER BY key is not in the DISTINCT output",
                );
                return None;
            }
            None => SortSource::Hidden(self.expr(key.expr)),
        };
        Some(Planned {
            source,
            desc: key.desc,
            nulls_first: key.nulls_first,
        })
    }

    /// The output position an ORDER BY key names: an ordinal, a lateral
    /// alias, or an expression equal to a select item.
    fn output_of(&self, key: ExprId, expanded: &Expanded) -> Option<usize> {
        let hir = self.hir;
        match hir.exprs.get(key).map(|e| &e.kind) {
            Some(ExprKind::Lit(Lit::Number(sym))) => {
                let n = hir.syms.text(*sym).parse::<usize>().ok()?.checked_sub(1)?;
                return (n < expanded.items.len()).then_some(n);
            }
            Some(ExprKind::Col(col)) => {
                let col = hir.cols.get(*col)?;
                if let Some(Resolution::LateralAlias(item)) = &col.res {
                    return expanded.item_of.get(*item as usize).copied().flatten();
                }
                if let Some(Resolution::Bound { from, col: index }) = &col.res {
                    let target = self.bindings.get(from)?.cols.get(*index as usize).copied();
                    let hit = expanded.items.iter().position(
                        |item| matches!(item, Item::Field(field) if Some(field.col) == target),
                    );
                    if hit.is_some() {
                        return hit;
                    }
                }
            }
            _ => {}
        }
        expanded
            .items
            .iter()
            .position(|item| matches!(item, Item::Expr { expr, .. } if same_expr(hir, *expr, key)))
    }
}
