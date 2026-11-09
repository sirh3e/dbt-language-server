//! Queries: CTE definitions, set operations, VALUES, holes, and the
//! ORDER BY and LIMIT of non-SELECT bodies.

use cogitator_ir::common::{HoleKind, Lit, SetOpKind};
use cogitator_ir::hir::{Body, Cte, CteId, ExprId, ExprKind, Query, QueryId, Resolution};
use cogitator_ir::mir::{ColId, CteDef, HirNode, ProvId, RelId, RelOp, ScalarId, SortKey};
use cogitator_ir::{ColType, DegradationKind, Span, Symbol};

use super::{Ctx, OpaqueSpec, MAX_QUERY_DEPTH};

/// A lowered query. `cols` is exactly `rel`'s output.
pub(super) struct QueryOut {
    pub(super) rel: RelId,
    pub(super) cols: Vec<ColId>,
    /// More columns may exist than `cols` names.
    pub(super) open: bool,
}

impl Ctx<'_> {
    pub(super) fn query(&mut self, q: QueryId, demand: &[Symbol]) -> QueryOut {
        let hir = self.hir;
        let Some(query) = hir.queries.get(q) else {
            self.bug(format!("query {q:?} is out of range"));
            let prov = self.prov(HirNode::None, Span::default());
            return self.hole(demand, HoleKind::Unsupported, Span::default(), prov);
        };
        let prov = self.prov(HirNode::Query(q), query.span);
        if self.query_depth >= MAX_QUERY_DEPTH || !self.tick() {
            self.cap(query.span, "this query");
            return self.hole(demand, HoleKind::Truncated, query.span, prov);
        }
        self.query_depth += 1;
        for cte in &query.ctes {
            self.cte(*cte);
        }
        let out = match &query.body {
            Body::Select(select) => self.select(q, query, select, demand),
            Body::SetOp {
                op,
                all,
                by_name,
                l,
                r,
            } => {
                let arms = self.flatten_set_op(*op, *all, *by_name, *l, *r);
                let out = self.set_op(*op, *all, *by_name, &arms, demand, query, prov);
                self.order_outputs(query, out, prov)
            }
            Body::Values(rows) => {
                let out = self.values(rows, query, demand, prov);
                self.order_outputs(query, out, prov)
            }
            Body::Hole(kind) => {
                self.degrade(
                    hole_degradation(*kind),
                    query.span,
                    "this query did not parse",
                );
                let out = self.hole(demand, *kind, query.span, prov);
                self.order_outputs(query, out, prov)
            }
        };
        let out = self.limit(query, out, prov);
        self.query_depth -= 1;
        out
    }

    /// An `Opaque` with no inputs, exposing only demanded columns.
    fn hole(&mut self, demand: &[Symbol], kind: HoleKind, span: Span, prov: ProvId) -> QueryOut {
        let spec = OpaqueSpec {
            cols: demand
                .iter()
                .map(|n| (Some(*n), ColType::UNKNOWN))
                .collect(),
            open: true,
            kind,
            span,
        };
        let (rel, cols) = self.opaque_rel(Vec::new(), spec, prov);
        QueryOut {
            rel,
            cols,
            open: true,
        }
    }

    /// Runs `q` in its own correlation frame. A body that reads outward
    /// cannot be a plan input (the verifier would see an unbound column),
    /// so it becomes an `Opaque` with the same columns.
    pub(super) fn isolated(
        &mut self,
        q: QueryId,
        demand: &[Symbol],
        span: Span,
        prov: ProvId,
    ) -> QueryOut {
        self.frames.push(Vec::new());
        let out = self.query(q, demand);
        let captured = self.frames.pop().unwrap_or_default();
        if captured.is_empty() {
            return out;
        }
        self.degrade(
            DegradationKind::UnsupportedSyntax,
            span,
            "a correlated CTE or LATERAL body is not modelled",
        );
        let spec = OpaqueSpec {
            cols: self.spec_of(&out.cols),
            open: out.open,
            kind: HoleKind::Unsupported,
            span,
        };
        let (rel, cols) = self.opaque_rel(Vec::new(), spec, prov);
        QueryOut {
            rel,
            cols,
            open: out.open,
        }
    }

    fn spec_of(&self, cols: &[ColId]) -> Vec<(Option<Symbol>, ColType)> {
        cols.iter()
            .map(|c| (self.col_name(*c), self.col_type(*c)))
            .collect()
    }

    fn cte(&mut self, id: CteId) {
        let hir = self.hir;
        let Some(cte) = hir.ctes.get(id) else {
            self.bug(format!("CTE {id:?} is out of range"));
            return;
        };
        let prov = self.prov(HirNode::Query(cte.query), cte.span);
        // boundary: the demand list outlives the mutable walk below
        let demand: Vec<Symbol> = self.demand.for_cte(id).to_vec();
        let out = if cte.recursive {
            self.recursive_cte(cte, &demand, prov)
        } else {
            let out = self.isolated(cte.query, &demand, cte.span, prov);
            self.rename(out, cte, prov)
        };
        let def = self.mir.ctes.alloc(CteDef {
            name: Some(cte.name.folded),
            body: out.rel,
            out: out.cols,
        });
        self.ctes.insert(id, def);
    }

    /// `Opaque`, keeping the anchor as an input; the recursive term is
    /// not lowered, so the CTE graph stays acyclic.
    fn recursive_cte(&mut self, cte: &Cte, demand: &[Symbol], prov: ProvId) -> QueryOut {
        let hir = self.hir;
        self.degrade(
            DegradationKind::RecursiveCte,
            cte.span,
            "a recursive CTE is opaque to the plan",
        );
        let anchor = match hir.queries.get(cte.query).map(|q| &q.body) {
            Some(Body::SetOp { l, .. }) => Some(self.isolated(*l, &[], cte.span, prov)),
            _ => None,
        };
        let mut cols: Vec<(Option<Symbol>, ColType)> = match &anchor {
            Some(a) => self.spec_of(&a.cols),
            None => Vec::new(),
        };
        for (i, name) in cte.columns.iter().enumerate() {
            match cols.get_mut(i) {
                Some(slot) => slot.0 = Some(name.folded),
                None => cols.push((Some(name.folded), ColType::UNKNOWN)),
            }
        }
        for name in demand {
            if !cols.iter().any(|(n, _)| *n == Some(*name)) {
                cols.push((Some(*name), ColType::UNKNOWN));
            }
        }
        let open = anchor.as_ref().is_none_or(|a| a.open);
        let inputs = anchor.iter().map(|a| a.rel).collect();
        let spec = OpaqueSpec {
            cols,
            open,
            kind: HoleKind::Unsupported,
            span: cte.span,
        };
        let (rel, cols) = self.opaque_rel(inputs, spec, prov);
        QueryOut { rel, cols, open }
    }

    /// Applies `WITH c(a, b)` names with a Project of fresh columns.
    fn rename(&mut self, out: QueryOut, cte: &Cte, prov: ProvId) -> QueryOut {
        if cte.columns.is_empty() {
            return out;
        }
        let mut items = Vec::with_capacity(out.cols.len());
        for (i, col) in out.cols.iter().enumerate() {
            let name = cte
                .columns
                .get(i)
                .map(|n| n.folded)
                .or_else(|| self.col_name(*col));
            let ty = self.col_type(*col);
            let fresh = self.mir.add_col(name, ty.clone());
            items.push((fresh, self.col_scalar(*col, ty, prov)));
        }
        let cols = items.iter().map(|(c, _)| *c).collect();
        let rel = self.mir.add_rel(
            RelOp::Project {
                input: out.rel,
                items,
            },
            prov,
        );
        QueryOut {
            rel,
            cols,
            open: out.open,
        }
    }

    /// `a UNION ALL b UNION ALL c` as one n-ary list, so long chains do
    /// not consume query depth. Only a bare left arm is absorbed.
    fn flatten_set_op(
        &self,
        op: SetOpKind,
        all: bool,
        by_name: bool,
        l: QueryId,
        r: QueryId,
    ) -> Vec<QueryId> {
        let hir = self.hir;
        let mut arms = vec![r];
        let mut left = l;
        while let Some(query) = hir.queries.get(left) {
            match &query.body {
                Body::SetOp {
                    op: inner,
                    all: inner_all,
                    by_name: inner_name,
                    l,
                    r,
                } if *inner == op
                    && *inner_all == all
                    && *inner_name == by_name
                    && query.ctes.is_empty()
                    && query.order_by.is_empty()
                    && query.limit.is_none()
                    && arms.len() < hir.queries.len() =>
                {
                    arms.push(*r);
                    left = *l;
                }
                _ => break,
            }
        }
        arms.push(left);
        arms.reverse();
        arms
    }

    #[allow(clippy::too_many_arguments)]
    fn set_op(
        &mut self,
        op: SetOpKind,
        all: bool,
        by_name: bool,
        arms: &[QueryId],
        demand: &[Symbol],
        query: &Query,
        prov: ProvId,
    ) -> QueryOut {
        let lowered: Vec<QueryOut> = arms.iter().map(|q| self.query(*q, demand)).collect();
        let inputs: Vec<RelId> = lowered.iter().map(|a| a.rel).collect();
        let first = lowered.first().map_or(&[][..], |a| a.cols.as_slice());
        let same_width = lowered.iter().all(|a| a.cols.len() == first.len());
        let open = lowered.iter().any(|a| a.open);
        let names_align = !by_name
            || lowered.iter().all(|a| {
                a.cols
                    .iter()
                    .zip(first)
                    .all(|(x, y)| self.col_name(*x) == self.col_name(*y))
            });
        if open || !same_width || !names_align {
            let span = query.span;
            let (kind, why) = if open {
                (
                    DegradationKind::OpenSchema,
                    "a set operation over an open arm",
                )
            } else {
                (
                    DegradationKind::UnsupportedSyntax,
                    "set operation arms do not line up by position",
                )
            };
            self.degrade(kind, span, why);
            let spec = OpaqueSpec {
                cols: self.spec_of(first),
                open: true,
                kind: HoleKind::Unsupported,
                span,
            };
            let (rel, cols) = self.opaque_rel(inputs, spec, prov);
            return QueryOut {
                rel,
                cols,
                open: true,
            };
        }
        let out: Vec<ColId> = (0..first.len())
            .map(|i| {
                let ty = lowered
                    .iter()
                    .map(|a| self.col_type(a.cols[i]))
                    .reduce(|acc, t| acc.join(&t))
                    .unwrap_or(ColType::UNKNOWN);
                let name = self.col_name(first[i]);
                self.mir.add_col(name, ty)
            })
            .collect();
        let rel = self.mir.add_rel(
            RelOp::SetOp {
                op,
                all,
                inputs,
                out: out.clone(),
            },
            prov,
        );
        QueryOut {
            rel,
            cols: out,
            open: false,
        }
    }

    /// One Project per row over the single-row source, unioned.
    fn values(
        &mut self,
        rows: &[Vec<ExprId>],
        query: &Query,
        demand: &[Symbol],
        prov: ProvId,
    ) -> QueryOut {
        let width = rows.first().map_or(0, Vec::len);
        if width == 0 || rows.iter().any(|r| r.len() != width) {
            self.degrade(
                DegradationKind::UnsupportedSyntax,
                query.span,
                "this VALUES list has no consistent shape",
            );
            return self.hole(demand, HoleKind::Unsupported, query.span, prov);
        }
        let name = |i: usize| query.outputs.get(i).and_then(|o| o.name);
        let mut arms = Vec::with_capacity(rows.len());
        for row in rows {
            let input = self.singleton(prov);
            let mut items = Vec::with_capacity(width);
            for (i, expr) in row.iter().enumerate() {
                let scalar = self.expr(*expr);
                let ty = self.scalar_type(scalar);
                let col = self.mir.add_col(name(i), ty);
                items.push((col, scalar));
            }
            let cols: Vec<ColId> = items.iter().map(|(c, _)| *c).collect();
            let rel = self.mir.add_rel(RelOp::Project { input, items }, prov);
            arms.push(QueryOut {
                rel,
                cols,
                open: false,
            });
        }
        if arms.len() == 1 {
            if let Some(only) = arms.pop() {
                return only;
            }
        }
        let inputs: Vec<RelId> = arms.iter().map(|a| a.rel).collect();
        let out: Vec<ColId> = (0..width)
            .map(|i| {
                let ty = arms
                    .iter()
                    .map(|a| self.col_type(a.cols[i]))
                    .reduce(|acc, t| acc.join(&t))
                    .unwrap_or(ColType::UNKNOWN);
                self.mir.add_col(name(i), ty)
            })
            .collect();
        let rel = self.mir.add_rel(
            RelOp::SetOp {
                op: SetOpKind::Union,
                all: true,
                inputs,
                out: out.clone(),
            },
            prov,
        );
        QueryOut {
            rel,
            cols: out,
            open: false,
        }
    }

    /// ORDER BY over named outputs only: ordinals and output names.
    pub(super) fn order_outputs(&mut self, query: &Query, out: QueryOut, prov: ProvId) -> QueryOut {
        if query.order_by.is_empty() {
            return out;
        }
        let mut keys = Vec::with_capacity(query.order_by.len());
        for key in &query.order_by {
            match self.output_position(key.expr, &out.cols) {
                Some(col) => {
                    let ty = self.col_type(col);
                    keys.push(SortKey {
                        expr: self.col_scalar(col, ty, prov),
                        desc: key.desc,
                        nulls_first: key.nulls_first,
                    });
                }
                None => {
                    let span = self.hir.exprs.get(key.expr).map_or(query.span, |e| e.span);
                    self.degrade(
                        DegradationKind::UnsupportedSyntax,
                        span,
                        "this ORDER BY key does not name an output column",
                    );
                }
            }
        }
        if keys.is_empty() {
            return out;
        }
        let rel = self.mir.add_rel(
            RelOp::Sort {
                input: out.rel,
                keys,
            },
            prov,
        );
        QueryOut { rel, ..out }
    }

    fn output_position(&self, key: ExprId, cols: &[ColId]) -> Option<ColId> {
        let hir = self.hir;
        match &hir.exprs.get(key)?.kind {
            ExprKind::Lit(Lit::Number(sym)) => {
                let n = hir.syms.text(*sym).parse::<usize>().ok()?.checked_sub(1)?;
                cols.get(n).copied()
            }
            ExprKind::Col(col) => {
                let col = hir.cols.get(*col)?;
                if let Some(Resolution::LateralAlias(i)) = col.res {
                    return cols.get(i as usize).copied();
                }
                let name = col.name.folded;
                cols.iter()
                    .copied()
                    .find(|c| self.col_name(*c) == Some(name))
            }
            _ => None,
        }
    }

    fn limit(&mut self, query: &Query, out: QueryOut, prov: ProvId) -> QueryOut {
        let Some(expr) = query.limit else {
            return out;
        };
        let scalar = self.limit_scalar(expr);
        let rel = self.mir.add_rel(
            RelOp::Limit {
                input: out.rel,
                limit: Some(scalar),
                offset: None,
            },
            prov,
        );
        QueryOut { rel, ..out }
    }

    /// A LIMIT cannot read columns; a reference is opaque, not a violation.
    fn limit_scalar(&mut self, expr: ExprId) -> ScalarId {
        let hir = self.hir;
        let not_literal = hir.exprs.get(expr).is_some_and(|e| {
            !matches!(
                e.kind,
                ExprKind::Lit(_) | ExprKind::Jinja(_) | ExprKind::Hole
            )
        });
        if not_literal {
            let span = hir.exprs.get(expr).map_or(Span::default(), |e| e.span);
            let prov = self.prov(HirNode::Expr(expr), span);
            self.degrade(
                DegradationKind::OpaqueExpression,
                span,
                "this LIMIT is not a literal",
            );
            return self.opaque_scalar(Vec::new(), HoleKind::Unsupported, span, prov);
        }
        self.expr(expr)
    }
}

fn hole_degradation(kind: HoleKind) -> DegradationKind {
    match kind {
        HoleKind::ParseError => DegradationKind::ParseHole,
        HoleKind::Unsupported => DegradationKind::UnsupportedSyntax,
        HoleKind::Jinja => DegradationKind::JinjaControlFlow,
        HoleKind::Truncated => DegradationKind::CapExceeded,
    }
}
