//! Queries, WITH clauses and set operations.

use cogitator_ir::common::{HoleKind, SetOpKind};
use cogitator_ir::hir::{
    Body, Cte, CteId, ExprId, ExprKind, FromKind, OrderItem, OutputCol, Query, QueryId, RelTarget,
    ScopeId, ScopeKind,
};
use cogitator_ir::{DegradationKind, Span};
use sqlparser::ast::{self, SetExpr, SetOperator, SetQuantifier};

use super::binder::{AliasMode, Frame};
use super::Lowerer;
use crate::parse::{Part, Parts};

/// What the body of a query produced.
pub(super) struct Lowered {
    pub body: Body,
    pub outputs: Vec<OutputCol>,
    pub open: bool,
    pub span: Span,
}

impl Lowerer<'_> {
    /// Lowers a full query (`WITH` + body + `ORDER BY` + `LIMIT`).
    pub(super) fn query(&mut self, q: &ast::Query, kind: ScopeKind, barrier: bool) -> QueryId {
        let parent = self.current_scope();
        let scope = self.alloc_scope(kind, parent, Span::default());
        self.frames.push(Frame::new(scope, barrier));
        let saved_mode = std::mem::replace(&mut self.mode, AliasMode::Off);
        let mark = self.visible.len();
        let ctes = self.with_clause(q.with.as_ref());
        self.check_pipes(q);
        // A parenthesised body with no clauses of its own is that query.
        if let SetExpr::Query(inner) = q.body.as_ref() {
            if q.order_by.is_none() && q.limit_clause.is_none() && q.fetch.is_none() {
                let id = self.query(inner, ScopeKind::Query, false);
                self.prepend_ctes(id, ctes);
                self.leave(mark, saved_mode);
                return id;
            }
        }
        let lowered = self.set_expr_body(q.body.as_ref());
        // ORDER BY sees input columns and outputs alike.
        self.bind_output_names(&lowered.outputs, lowered.open);
        self.mode = AliasMode::OrderBy;
        let order_by = self.order_by(q.order_by.as_ref());
        self.mode = AliasMode::Off;
        let limit = self.limit_clause(q);
        let id = self.alloc_query(
            Query {
                span: lowered.span,
                scope,
                ctes,
                body: lowered.body,
                order_by,
                limit,
                outputs: lowered.outputs,
            },
            lowered.open,
        );
        self.finish_scope(scope, lowered.span);
        self.leave(mark, saved_mode);
        id
    }

    fn check_pipes(&mut self, q: &ast::Query) {
        if !q.pipe_operators.is_empty() {
            self.degrade(
                DegradationKind::UnsupportedSyntax,
                Span::default(),
                "the pipe operator is not modelled",
            );
        }
    }

    fn leave(&mut self, mark: usize, mode: AliasMode) {
        self.visible.truncate(mark);
        self.frames.pop();
        self.mode = mode;
    }

    fn finish_scope(&mut self, scope: ScopeId, span: Span) {
        let items = self
            .frames
            .last()
            .map(|f| f.items.clone())
            .unwrap_or_default();
        if let Some(s) = self.hir.scopes.get_mut(scope) {
            s.span = span;
            s.from = items;
        }
    }

    /// Output names become aliases for ORDER BY; a set operation's
    /// frame has no inputs, so unresolved names stay quiet if open.
    fn bind_output_names(&mut self, outputs: &[OutputCol], open: bool) {
        if let Some(frame) = self.frames.last_mut() {
            frame.open_outputs = open && frame.items.is_empty();
            frame.aliases = outputs
                .iter()
                .enumerate()
                .filter_map(|(i, col)| col.name.map(|name| (name, i as u32)))
                .collect();
        }
    }

    fn with_clause(&mut self, with: Option<&ast::With>) -> Vec<CteId> {
        let Some(with) = with else { return Vec::new() };
        let mut ids = Vec::with_capacity(with.cte_tables.len());
        for cte in &with.cte_tables {
            let columns: Vec<&ast::Ident> = cte.alias.columns.iter().map(|c| &c.name).collect();
            ids.push(self.cte(&cte.alias.name, &columns, with.recursive, |this| {
                this.query(&cte.query, ScopeKind::Cte, true)
            }));
        }
        ids
    }

    pub(super) fn cte_parts(&mut self, parts: &Parts) -> Vec<CteId> {
        let mut ids = Vec::with_capacity(parts.ctes.len());
        for cte in &parts.ctes {
            let columns: Vec<&ast::Ident> = cte.columns.iter().collect();
            ids.push(
                self.cte(&cte.name, &columns, parts.recursive, |this| match &cte.body {
                    Part::Parsed(query) => this.query(query, ScopeKind::Cte, true),
                    Part::Hole(kind, span) => this.hole_query(*kind, *span, Vec::new()),
                }),
            );
        }
        ids
    }

    /// Declares one CTE around the lowering of its body. A `WITH RECURSIVE`
    /// entry is visible to itself and marked recursive only if the body
    /// really refers to it.
    fn cte(
        &mut self,
        name: &ast::Ident,
        columns: &[&ast::Ident],
        recursive: bool,
        body: impl FnOnce(&mut Self) -> QueryId,
    ) -> CteId {
        let name = self.ident(name);
        let columns: Vec<_> = columns.iter().map(|c| self.ident(c)).collect();
        // A placeholder query id; patched below once the body exists.
        let id = self.hir.ctes.alloc(Cte {
            span: name.span,
            name,
            columns,
            query: QueryId::from_raw(u32::MAX),
            recursive: false,
        });
        let first_from = self.hir.from.len();
        if recursive {
            self.visible.push((name.folded, id));
        }
        let query = body(self);
        let referenced = recursive && self.references_cte(first_from, id);
        if let Some(cte) = self.hir.ctes.get_mut(id) {
            cte.query = query;
            cte.recursive = referenced;
        }
        if !recursive {
            self.visible.push((name.folded, id));
        }
        id
    }

    /// True if any FROM item allocated since `first_from` targets `cte`.
    fn references_cte(&self, first_from: usize, cte: CteId) -> bool {
        self.hir.from.iter().skip(first_from).any(|(_, item)| {
            matches!(&item.kind, FromKind::Base { target: RelTarget::Cte(c), .. } if *c == cte)
        })
    }

    fn set_expr_body(&mut self, body: &SetExpr) -> Lowered {
        match body {
            SetExpr::Select(select) => self.select(select),
            SetExpr::SetOperation {
                left,
                op,
                set_quantifier,
                right,
            } => self.set_operation(left, *op, *set_quantifier, right),
            SetExpr::Values(values) => self.values(values),
            SetExpr::Query(_) => {
                self.unsupported_body("a parenthesised query with its own ORDER BY or LIMIT")
            }
            SetExpr::Table(_) => self.unsupported_body("a TABLE statement"),
            SetExpr::Insert(_) | SetExpr::Update(_) | SetExpr::Delete(_) | SetExpr::Merge(_) => {
                self.unsupported_body("a DML statement")
            }
        }
    }

    fn unsupported_body(&mut self, what: &str) -> Lowered {
        self.degrade(
            DegradationKind::UnsupportedSyntax,
            Span::default(),
            format!("{what} is not modelled"),
        );
        Lowered {
            body: Body::Hole(HoleKind::Unsupported),
            outputs: Vec::new(),
            open: true,
            span: Span::default(),
        }
    }

    fn set_operation(
        &mut self,
        left: &SetExpr,
        op: SetOperator,
        quantifier: SetQuantifier,
        right: &SetExpr,
    ) -> Lowered {
        let l = self.set_arm(left);
        let r = self.set_arm(right);
        let (all, by_name) = quantifier_flags(quantifier);
        let op = match op {
            SetOperator::Union => SetOpKind::Union,
            SetOperator::Intersect => SetOpKind::Intersect,
            SetOperator::Except | SetOperator::Minus => SetOpKind::Except,
        };
        let (outputs, open) = self.set_outputs(l, r, by_name);
        let span = self.arms_span(l, r);
        Lowered {
            body: Body::SetOp {
                op,
                all,
                by_name,
                l,
                r,
            },
            outputs,
            open,
            span,
        }
    }

    fn arms_span(&self, l: QueryId, r: QueryId) -> Span {
        match (self.hir.queries.get(l), self.hir.queries.get(r)) {
            (Some(a), Some(b)) => super::union(a.span, b.span),
            _ => Span::default(),
        }
    }

    /// One arm: a parenthesised query keeps its clauses, a bare SELECT
    /// is wrapped in its own query node and scope.
    fn set_arm(&mut self, arm: &SetExpr) -> QueryId {
        match arm {
            SetExpr::Query(q) => self.query(q, ScopeKind::Query, false),
            other => {
                let parent = self.current_scope();
                let scope = self.alloc_scope(ScopeKind::Query, parent, Span::default());
                self.frames.push(Frame::new(scope, false));
                let saved = std::mem::replace(&mut self.mode, AliasMode::Off);
                let lowered = self.set_expr_body(other);
                self.finish_scope(scope, lowered.span);
                self.frames.pop();
                self.mode = saved;
                self.alloc_query(
                    Query {
                        span: lowered.span,
                        scope,
                        ctes: Vec::new(),
                        body: lowered.body,
                        order_by: Vec::new(),
                        limit: None,
                        outputs: lowered.outputs,
                    },
                    lowered.open,
                )
            }
        }
    }

    /// The left arm's columns; `BY NAME` appends right-only names.
    fn set_outputs(&mut self, l: QueryId, r: QueryId, by_name: bool) -> (Vec<OutputCol>, bool) {
        let open = self.query_is_open(l) || self.query_is_open(r);
        let Some(left) = self.hir.queries.get(l) else {
            return (Vec::new(), true);
        };
        // boundary: outputs are owned per query; arms and the set op differ
        let mut outputs = left.outputs.clone();
        if let (true, Some(right)) = (by_name, self.hir.queries.get(r)) {
            for col in &right.outputs {
                let present = col.name.is_some() && outputs.iter().any(|o| o.name == col.name);
                if !present {
                    outputs.push(col.clone());
                }
            }
        }
        self.charge(outputs.len());
        (outputs, open)
    }

    pub(super) fn query_is_open(&self, q: QueryId) -> bool {
        self.query_open.get(q.index()).copied().unwrap_or(true)
    }

    fn values(&mut self, values: &ast::Values) -> Lowered {
        let rows: Vec<Vec<ExprId>> = values
            .rows
            .iter()
            .map(|row| row.iter().map(|e| self.expr(e)).collect())
            .collect();
        let arity = rows.first().map_or(0, Vec::len);
        let outputs = (0..arity)
            .map(|_| Self::output_col(Span::default(), None, None))
            .collect();
        let all: Vec<ExprId> = rows.iter().flatten().copied().collect();
        let span = self.union_spans(&all);
        Lowered {
            body: Body::Values(rows),
            outputs,
            open: false,
            span,
        }
    }

    fn order_by(&mut self, order: Option<&ast::OrderBy>) -> Vec<OrderItem> {
        match order.map(|o| &o.kind) {
            None => Vec::new(),
            Some(ast::OrderByKind::All(options)) => {
                let expr = self.alloc_expr(
                    Span::default(),
                    ExprKind::Opaque {
                        children: Vec::new(),
                    },
                );
                vec![OrderItem {
                    expr,
                    desc: sort_desc(options),
                    nulls_first: options.nulls_first,
                }]
            }
            Some(ast::OrderByKind::Expressions(items)) => self.order_items(items),
        }
    }

    pub(super) fn order_items(&mut self, items: &[ast::OrderByExpr]) -> Vec<OrderItem> {
        items
            .iter()
            .map(|item| OrderItem {
                expr: self.expr(&item.expr),
                desc: sort_desc(&item.options),
                nulls_first: item.options.nulls_first,
            })
            .collect()
    }

    fn limit_clause(&mut self, q: &ast::Query) -> Option<ExprId> {
        match &q.limit_clause {
            Some(ast::LimitClause::LimitOffset { limit, .. }) => {
                limit.as_ref().map(|e| self.expr(e))
            }
            Some(ast::LimitClause::OffsetCommaLimit { limit, .. }) => Some(self.expr(limit)),
            None => q
                .fetch
                .as_ref()
                .and_then(|f| f.quantity.as_ref())
                .map(|e| self.expr(e)),
        }
    }
}

fn sort_desc(options: &ast::OrderByOptions) -> Option<bool> {
    match options.sort {
        Some(ast::OrderBySort::Asc) => Some(false),
        Some(ast::OrderBySort::Desc) => Some(true),
        Some(ast::OrderBySort::Using(_)) | None => None,
    }
}

fn quantifier_flags(q: SetQuantifier) -> (bool, bool) {
    match q {
        SetQuantifier::All => (true, false),
        SetQuantifier::Distinct | SetQuantifier::None => (false, false),
        SetQuantifier::ByName | SetQuantifier::DistinctByName => (false, true),
        SetQuantifier::AllByName => (true, true),
    }
}
