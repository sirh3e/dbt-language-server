//! FROM items: each leaf is pushed into the current frame so later
//! items and ON conditions can see it.

use std::sync::Arc;

use cogitator_ir::common::JoinKind;
use cogitator_ir::hir::{FromCol, FromId, FromKind, Ident, JinjaRole, JoinCond, QueryId, RelTarget, ScopeKind};
use cogitator_ir::schema::{Completeness, ModelSchema};
use cogitator_ir::{ColType, DegradationKind, Span, Symbol};
use sqlparser::ast::{self, JoinConstraint, JoinOperator, TableAlias, TableFactor};

use super::binder::AliasMode;
use super::{union, Lowerer};
use crate::jinja::{Class, RelInfo};

impl Lowerer<'_> {
    pub(super) fn from_items(&mut self, tables: &[ast::TableWithJoins]) -> Vec<FromId> {
        tables.iter().map(|t| self.table_with_joins(t)).collect()
    }

    fn table_with_joins(&mut self, table: &ast::TableWithJoins) -> FromId {
        let mut left = self.table_factor(&table.relation);
        for join in &table.joins {
            let right = self.table_factor(&join.relation);
            let (kind, constraint) = join_kind(&join.join_operator);
            let on = self.join_cond(constraint);
            let span = self.from_span(left, right);
            left = self.alloc_from(
                span,
                FromKind::Join {
                    kind,
                    l: left,
                    r: right,
                    on,
                },
                Vec::new(),
                false,
                Vec::new(),
            );
        }
        left
    }

    fn from_span(&self, l: FromId, r: FromId) -> Span {
        match (self.hir.from.get(l), self.hir.from.get(r)) {
            (Some(a), Some(b)) => union(a.span, b.span),
            _ => Span::default(),
        }
    }

    fn join_cond(&mut self, constraint: Option<&JoinConstraint>) -> JoinCond {
        match constraint {
            None | Some(JoinConstraint::None) => JoinCond::None,
            Some(JoinConstraint::Natural) => JoinCond::Natural,
            Some(JoinConstraint::On(e)) => JoinCond::On(self.expr(e)),
            Some(JoinConstraint::Using(names)) => {
                let idents: Vec<Ident> = names
                    .iter()
                    .filter_map(|n| match n.0.last()? {
                        ast::ObjectNamePart::Identifier(i) => Some(self.ident(i)),
                        ast::ObjectNamePart::Function(_) => None,
                    })
                    .collect();
                let using: Vec<Symbol> = idents.iter().map(|i| i.folded).collect();
                if let Some(frame) = self.frames.last_mut() {
                    frame.using.extend(using);
                }
                JoinCond::Using(idents)
            }
        }
    }

    fn table_factor(&mut self, factor: &TableFactor) -> FromId {
        match factor {
            TableFactor::Table {
                name, alias, args, ..
            } => match args {
                Some(args) => self.table_fn(name, &args.args, alias.as_ref()),
                None => self.base_table(name, alias.as_ref()),
            },
            TableFactor::Derived {
                lateral,
                subquery,
                alias,
                ..
            } => self.derived(*lateral, subquery, alias.as_ref()),
            TableFactor::TableFunction { expr, alias } => self.table_function(expr, alias.as_ref()),
            TableFactor::Function {
                name, args, alias, ..
            } => self.table_fn(name, args, alias.as_ref()),
            TableFactor::UNNEST {
                alias,
                array_exprs,
                with_offset,
                with_offset_alias,
                ..
            } => self.unnest(array_exprs, *with_offset, with_offset_alias.as_ref(), alias.as_ref()),
            TableFactor::NestedJoin {
                table_with_joins,
                alias,
            } => {
                if alias.is_some() {
                    self.degrade(
                        DegradationKind::UnsupportedSyntax,
                        Span::default(),
                        "an alias on a parenthesised join is not modelled",
                    );
                }
                self.table_with_joins(table_with_joins)
            }
            other => self.unsupported_factor(other),
        }
    }

    fn unsupported_factor(&mut self, factor: &TableFactor) -> FromId {
        self.degrade(
            DegradationKind::UnsupportedSyntax,
            Span::default(),
            "this table factor (PIVOT, JSON_TABLE, …) is not modelled",
        );
        // The source relation stays analysable as a sibling of the hole.
        if let TableFactor::Pivot { table, .. }
        | TableFactor::Unpivot { table, .. }
        | TableFactor::MatchRecognize { table, .. } = factor
        {
            self.table_factor(table);
        }
        let names = factor_alias(factor)
            .map(|a| vec![self.sym_for(&a.name)])
            .unwrap_or_default();
        self.leaf(Span::default(), FromKind::Hole, Vec::new(), true, names)
    }

    /// Allocates a leaf and makes it visible in the current frame.
    fn leaf(
        &mut self,
        span: Span,
        kind: FromKind,
        output: Vec<FromCol>,
        open: bool,
        names: Vec<Symbol>,
    ) -> FromId {
        let id = self.alloc_from(span, kind, output, open, names);
        if let Some(frame) = self.frames.last_mut() {
            frame.items.push(id);
        }
        id
    }

    fn base_table(&mut self, name: &ast::ObjectName, alias: Option<&TableAlias>) -> FromId {
        let path = name_idents(name);
        if path.is_empty() {
            return self.leaf(Span::default(), FromKind::Hole, Vec::new(), true, Vec::new());
        }
        let alias_ident = alias.and_then(|a| self.alias_ident(&a.name));
        if let Some(region) = self.region_in_path(&path) {
            return self.jinja_base(region, alias_ident, alias, &path);
        }
        let span = self.path_span(&path);
        if let [single] = path.as_slice() {
            let folded = self.sym_for(single);
            if let Some(&(_, cte)) = self.visible.iter().rev().find(|(n, _)| *n == folded) {
                return self.cte_base(span, cte, alias_ident, alias, folded);
            }
        }
        let idents: Vec<Ident> = path.iter().map(|i| self.ident(i)).collect();
        let names = alias_ident
            .map(|a| vec![a.folded])
            .unwrap_or_else(|| idents.iter().map(|i| i.folded).collect());
        let output = alias_columns(alias).map_or_else(Vec::new, |cols| self.column_set(cols));
        self.leaf(
            span,
            FromKind::Base {
                target: RelTarget::Physical(idents),
                alias: alias_ident,
            },
            output,
            true,
            names,
        )
    }

    fn cte_base(
        &mut self,
        span: Span,
        cte: cogitator_ir::hir::CteId,
        alias: Option<Ident>,
        alias_ast: Option<&TableAlias>,
        name: Symbol,
    ) -> FromId {
        let (mut output, open) = self.cte_output(cte);
        if let Some(cols) = alias_columns(alias_ast) {
            output = self.apply_columns(cols, output);
        }
        let names = vec![alias.map_or(name, |a| a.folded)];
        self.leaf(
            span,
            FromKind::Base {
                target: RelTarget::Cte(cte),
                alias,
            },
            output,
            open,
            names,
        )
    }

    fn cte_output(&mut self, cte: cogitator_ir::hir::CteId) -> (Vec<FromCol>, bool) {
        let Some(entry) = self.hir.ctes.get(cte) else {
            return (Vec::new(), true);
        };
        let query = entry.query;
        // A self-reference is seen before its body exists: stay open.
        if self.hir.queries.get(query).is_none() {
            return (Vec::new(), true);
        }
        // boundary: per-reference copy of the CTE's column list
        let columns = entry.columns.iter().map(|c| c.folded).collect::<Vec<_>>();
        let (output, open) = self.query_output(query);
        let output = if columns.is_empty() {
            output
        } else {
            columns
                .into_iter()
                .enumerate()
                .map(|(i, name)| FromCol {
                    name,
                    ty: output.get(i).map_or(ColType::UNKNOWN, |c| c.ty.clone()),
                })
                .collect()
        };
        (output, open)
    }

    fn query_output(&self, query: QueryId) -> (Vec<FromCol>, bool) {
        let open = self.query_is_open(query);
        let outputs = self
            .hir
            .queries
            .get(query)
            .map(|q| {
                q.outputs
                    .iter()
                    .filter_map(|col| {
                        col.name.map(|name| FromCol {
                            name,
                            ty: col.ty.clone(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        (outputs, open)
    }

    fn jinja_base(
        &mut self,
        region: usize,
        alias: Option<Ident>,
        alias_ast: Option<&TableAlias>,
        path: &[ast::Ident],
    ) -> FromId {
        let span = self.region_span(region);
        let class = self.region(region).map(|r| r.class.clone());
        let static_rel = match class {
            Some(Class::Relation(rel)) if rel != RelInfo::Dynamic => Some(rel),
            _ => None,
        };
        match static_rel {
            Some(RelInfo::Model {
                package,
                name,
                version,
            }) => {
                let target = RelTarget::Model {
                    package: package.map(|p| self.intern(&p)),
                    name: self.intern(&name),
                    version: version.map(|v| self.intern(&v)),
                };
                let schema = self.env.schemas.model(&name).cloned();
                let folded = self.rules.fold_name(&name);
                let folded = self.intern(&folded);
                self.static_base(span, target, alias, alias_ast, folded, schema)
            }
            Some(RelInfo::Source { source, table }) => {
                let schema = self.env.schemas.source(&source, &table).cloned();
                let target = RelTarget::Source {
                    source: self.intern(&source),
                    table: self.intern(&table),
                };
                let folded = self.rules.fold_name(&table);
                let folded = self.intern(&folded);
                self.static_base(span, target, alias, alias_ast, folded, schema)
            }
            Some(RelInfo::This) => {
                let model = self.env.model.clone();
                let schema = model.as_deref().and_then(|m| self.env.schemas.model(m).cloned());
                let folded = model
                    .as_deref()
                    .map(|m| self.rules.fold_name(m).into_owned())
                    .unwrap_or_default();
                let folded = self.intern(&folded);
                self.static_base(span, RelTarget::This, alias, alias_ast, folded, schema)
            }
            _ => self.dynamic_base(region, span, alias, path),
        }
    }

    fn static_base(
        &mut self,
        span: Span,
        target: RelTarget,
        alias: Option<Ident>,
        alias_ast: Option<&TableAlias>,
        name: Symbol,
        schema: Option<Arc<ModelSchema>>,
    ) -> FromId {
        let (mut output, open) = self.schema_output(schema.as_deref());
        if let Some(cols) = alias_columns(alias_ast) {
            output = self.apply_columns(cols, output);
        }
        let names = vec![alias.map_or(name, |a| a.folded)];
        self.leaf(span, FromKind::Base { target, alias }, output, open, names)
    }

    fn schema_output(&mut self, schema: Option<&ModelSchema>) -> (Vec<FromCol>, bool) {
        let Some(schema) = schema else {
            return (Vec::new(), true);
        };
        let output = schema
            .columns
            .iter()
            .map(|c| FromCol {
                name: {
                    let folded = self.rules.fold_name(&c.name);
                    self.hir.syms.intern(&folded)
                },
                ty: c.ty.clone(), // boundary: the plan owns its column types
            })
            .collect();
        (output, schema.completeness != Completeness::Closed)
    }

    /// `{{ my_macro() }}` or `ref(var('x'))`: known only as a relation.
    fn dynamic_base(
        &mut self,
        region: usize,
        span: Span,
        alias: Option<Ident>,
        path: &[ast::Ident],
    ) -> FromId {
        let names = alias.map(|a| vec![a.folded]).unwrap_or_else(|| {
            path.last()
                .filter(|i| self.region_at(self.ident_span(i)).is_none())
                .map(|i| vec![self.sym_for(i)])
                .unwrap_or_default()
        });
        match self.take_region(region, JinjaRole::Relation) {
            Some(jinja) => self.leaf(
                span,
                FromKind::Base {
                    target: RelTarget::Jinja(jinja),
                    alias,
                },
                Vec::new(),
                true,
                names,
            ),
            None => self.leaf(span, FromKind::Hole, Vec::new(), true, names),
        }
    }

    fn derived(&mut self, lateral: bool, subquery: &ast::Query, alias: Option<&TableAlias>) -> FromId {
        let saved = std::mem::replace(&mut self.mode, AliasMode::Off);
        let q = self.query(subquery, ScopeKind::Derived, !lateral);
        self.mode = saved;
        let (mut output, open) = self.query_output(q);
        if let Some(cols) = alias_columns(alias) {
            output = self.apply_columns(cols, output);
        }
        let alias_ident = alias.and_then(|a| self.alias_ident(&a.name));
        let names = alias_ident.map(|a| vec![a.folded]).unwrap_or_default();
        let span = self.hir.queries.get(q).map_or(Span::default(), |q| q.span);
        self.leaf(
            span,
            FromKind::Derived {
                q,
                lateral,
                alias: alias_ident,
            },
            output,
            open,
            names,
        )
    }

    fn table_function(&mut self, expr: &ast::Expr, alias: Option<&TableAlias>) -> FromId {
        if let ast::Expr::Function(f) = expr {
            let args = match &f.args {
                ast::FunctionArguments::List(list) => list.args.as_slice(),
                _ => &[],
            };
            return self.table_fn(&f.name, args, alias);
        }
        let saved = std::mem::replace(&mut self.mode, AliasMode::Off);
        let child = self.expr(expr);
        self.mode = saved;
        let span = self.hir.exprs.get(child).map_or(Span::default(), |e| e.span);
        self.open_table_fn(span, Vec::new(), vec![child], alias)
    }

    fn table_fn(
        &mut self,
        name: &ast::ObjectName,
        args: &[ast::FunctionArg],
        alias: Option<&TableAlias>,
    ) -> FromId {
        let path = name_idents(name);
        let name_idents: Vec<Ident> = path.iter().map(|i| self.ident(i)).collect();
        let span = self.path_span(&path);
        let saved = std::mem::replace(&mut self.mode, AliasMode::Off);
        let args = self.function_arg_list(args);
        self.mode = saved;
        self.open_table_fn(span, name_idents, args, alias)
    }

    fn open_table_fn(
        &mut self,
        span: Span,
        name: Vec<Ident>,
        args: Vec<cogitator_ir::hir::ExprId>,
        alias: Option<&TableAlias>,
    ) -> FromId {
        let alias_ident = alias.and_then(|a| self.alias_ident(&a.name));
        let names = alias_ident.map(|a| vec![a.folded]).unwrap_or_default();
        let output = alias_columns(alias).map_or_else(Vec::new, |cols| self.column_set(cols));
        self.leaf(
            span,
            FromKind::TableFn {
                name,
                args,
                alias: alias_ident,
            },
            output,
            true,
            names,
        )
    }

    fn unnest(
        &mut self,
        exprs: &[ast::Expr],
        with_offset: bool,
        offset_alias: Option<&ast::Ident>,
        alias: Option<&TableAlias>,
    ) -> FromId {
        let saved = std::mem::replace(&mut self.mode, AliasMode::Off);
        let lowered: Vec<_> = exprs.iter().map(|e| self.expr(e)).collect();
        self.mode = saved;
        let span = self.union_spans(&lowered);
        let alias_ident = alias.and_then(|a| self.alias_ident(&a.name));
        let names = alias_ident.map(|a| vec![a.folded]).unwrap_or_default();
        // `unnest(x) as v` names the element column; `as t(v)` the columns.
        let mut output = match alias_columns(alias) {
            Some(cols) => self.column_set(cols),
            None => alias_ident
                .map(|a| {
                    vec![FromCol {
                        name: a.folded,
                        ty: ColType::UNKNOWN,
                    }]
                })
                .unwrap_or_default(),
        };
        if let Some(offset) = offset_alias {
            output.push(FromCol {
                name: self.sym_for(offset),
                ty: ColType::UNKNOWN,
            });
        }
        self.leaf(
            span,
            FromKind::Unnest {
                exprs: lowered,
                with_offset,
                alias: alias_ident,
            },
            output,
            true,
            names,
        )
    }

    fn column_set(&mut self, cols: &[ast::TableAliasColumnDef]) -> Vec<FromCol> {
        cols.iter()
            .map(|c| FromCol {
                name: self.sym_for(&c.name),
                ty: ColType::UNKNOWN,
            })
            .collect()
    }

    /// Positional rename by a table alias column list.
    fn apply_columns(
        &mut self,
        cols: &[ast::TableAliasColumnDef],
        mut output: Vec<FromCol>,
    ) -> Vec<FromCol> {
        for (i, def) in cols.iter().enumerate() {
            let name = self.sym_for(&def.name);
            match output.get_mut(i) {
                Some(col) => col.name = name,
                None => output.push(FromCol {
                    name,
                    ty: ColType::UNKNOWN,
                }),
            }
        }
        output
    }
}

fn alias_columns(alias: Option<&TableAlias>) -> Option<&[ast::TableAliasColumnDef]> {
    alias
        .map(|a| a.columns.as_slice())
        .filter(|cols| !cols.is_empty())
}

fn name_idents(name: &ast::ObjectName) -> Vec<ast::Ident> {
    name.0
        .iter()
        .filter_map(|part| match part {
            // boundary: a short path, compared and interned, never stored
            ast::ObjectNamePart::Identifier(i) => Some(i.clone()),
            ast::ObjectNamePart::Function(_) => None,
        })
        .collect()
}

fn factor_alias(factor: &TableFactor) -> Option<&TableAlias> {
    match factor {
        TableFactor::Pivot { alias, .. }
        | TableFactor::Unpivot { alias, .. }
        | TableFactor::MatchRecognize { alias, .. }
        | TableFactor::JsonTable { alias, .. }
        | TableFactor::OpenJsonTable { alias, .. }
        | TableFactor::XmlTable { alias, .. }
        | TableFactor::SemanticView { alias, .. } => alias.as_ref(),
        _ => None,
    }
}

/// Maps a join operator to the HIR kind and its constraint.
fn join_kind(op: &JoinOperator) -> (JoinKind, Option<&JoinConstraint>) {
    use JoinOperator as J;
    match op {
        J::Join(c) | J::Inner(c) | J::StraightJoin(c) => (JoinKind::Inner, Some(c)),
        J::Left(c) | J::LeftOuter(c) => (JoinKind::Left, Some(c)),
        J::Right(c) | J::RightOuter(c) => (JoinKind::Right, Some(c)),
        J::FullOuter(c) => (JoinKind::Full, Some(c)),
        J::CrossJoin(c) => (JoinKind::Cross, Some(c)),
        J::Semi(c) | J::LeftSemi(c) | J::RightSemi(c) => (JoinKind::Semi, Some(c)),
        J::Anti(c) | J::LeftAnti(c) | J::RightAnti(c) => (JoinKind::Anti, Some(c)),
        J::CrossApply => (JoinKind::Cross, None),
        J::OuterApply => (JoinKind::Left, None),
        J::AsOf { constraint, .. } => (JoinKind::Left, Some(constraint)),
        J::ArrayJoin | J::LeftArrayJoin | J::InnerArrayJoin => (JoinKind::Cross, None),
    }
}
