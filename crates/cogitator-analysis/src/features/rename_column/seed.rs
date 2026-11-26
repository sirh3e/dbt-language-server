//! Where a model defines one of its output columns: from the root query
//! down through `select * from cte` and bare `cte.col` pass-throughs to
//! the item that names it. An alias there is renamed (`x as new`); a bare
//! column of an upstream relation gets an alias (`col as new`), so the
//! upstream model is never touched. CTE-internal names above that item
//! change only because the walk carries the new name up to the root.

use cogitator_ir::hir::{Body, FromId, QueryId, SelectItem};

use super::sql::{Found, SqlEdit, Walk};

/// How a query provides the column.
enum Provides {
    Alias(usize, cogitator_ir::hir::Ident),
    /// A bare column: the in-model query it reads, if it reads one.
    Bare(
        usize,
        Option<QueryId>,
        cogitator_ir::hir::Ident,
        cogitator_ir::Span,
        Option<FromId>,
    ),
    Star(QueryId),
    /// Through `*` over a relation outside the model: the FROM item.
    UpstreamStar(FromId),
}

/// Where a model's output column comes from, for an upstream rename.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Defined {
    /// An item of the model names it (an alias, or a bare column whose
    /// FROM item, when it is outside the model, is given).
    Item(Option<FromId>),
    /// Only `*` over these FROM items outside the model passes it.
    Stars(Vec<FromId>),
}

impl Walk<'_> {
    pub(super) fn find_output(&self, query: QueryId) -> Result<Found, String> {
        match self.define(query)? {
            (Some(found), _) => Ok(found),
            (None, _) => Err(format!(
                "`{}` comes in through `select *` from an upstream relation; rename it there",
                self.column
            )),
        }
    }

    /// How the model's output defines the column, for an upstream rename.
    pub(super) fn defined(&self, query: QueryId) -> Result<Defined, String> {
        Ok(match self.define(query)? {
            (Some(_), Defined::Item(from)) => Defined::Item(from),
            (_, defined) => defined,
        })
    }

    /// The defining item (none when only an upstream `*` passes it) and
    /// what it reads.
    fn define(&self, query: QueryId) -> Result<(Option<Found>, Defined), String> {
        let column = self.column;
        let mut depth = 0;
        let mut query = query;
        loop {
            depth += 1;
            if depth > 64 {
                return Err("its CTEs nest too deeply".into());
            }
            let provided = self.provides(query)?;
            let explicit: Vec<&Provides> = provided
                .iter()
                .filter(|p| !matches!(p, Provides::UpstreamStar(_)))
                .collect();
            match explicit.as_slice() {
                [] if !provided.is_empty() => {
                    let stars = provided
                        .iter()
                        .filter_map(|p| match p {
                            Provides::UpstreamStar(from) => Some(*from),
                            _ => None,
                        })
                        .collect();
                    return Ok((None, Defined::Stars(stars)));
                }
                [] => return Err(format!("it outputs no column named `{column}`")),
                [one] => match one {
                    Provides::Alias(item, alias) => {
                        let found = Found {
                            query,
                            item: *item,
                            site: self.site(*alias, SqlEdit::Name),
                        };
                        return Ok((Some(found), Defined::Item(None)));
                    }
                    Provides::Bare(_, Some(inner), _, _, _) | Provides::Star(inner) => {
                        query = *inner;
                    }
                    Provides::Bare(item, None, name, span, from) => {
                        let mut site = self.site(*name, SqlEdit::AliasAfter);
                        site.span = *span;
                        let found = Found {
                            query,
                            item: *item,
                            site,
                        };
                        return Ok((Some(found), Defined::Item(*from)));
                    }
                    Provides::UpstreamStar(_) => unreachable!("filtered out above"),
                },
                _ => return Err(format!("it outputs `{column}` more than once")),
            }
        }
    }

    fn provides(&self, query: QueryId) -> Result<Vec<Provides>, String> {
        let q = self.hir.queries.get(query).ok_or("a query is missing")?;
        let Body::Select(select) = &q.body else {
            return Err(format!(
                "`{}` comes out of a set operation or VALUES; rename it by hand",
                self.column
            ));
        };
        let mut out = Vec::new();
        for (index, item) in select.items.iter().enumerate() {
            match item {
                SelectItem::Expr {
                    alias: Some(alias), ..
                } if self.is(*alias) => out.push(Provides::Alias(index, *alias)),
                SelectItem::Expr { expr, alias: None } => {
                    let Some(col) = self.bare_col(*expr).filter(|c| self.is(c.name)) else {
                        continue;
                    };
                    let from = Self::direct(col.res.as_ref());
                    let inner = from
                        .and_then(|f| self.inner_query(f))
                        .filter(|q| self.outputs(*q, 0));
                    out.push(Provides::Bare(index, inner, col.name, col.span, from));
                }
                SelectItem::Star { from, .. } => {
                    for target in self.star_targets(query, *from) {
                        match self.inner_query(target) {
                            Some(inner) if self.outputs(inner, 0) => {
                                out.push(Provides::Star(inner))
                            }
                            Some(inner) => out.extend(
                                self.upstream_stars(inner, 0)
                                    .into_iter()
                                    .map(Provides::UpstreamStar),
                            ),
                            None => out.push(Provides::UpstreamStar(target)),
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(out)
    }

    /// The relations outside the model an in-model query's `*` items
    /// reach, through CTEs and derived tables that do not name the column.
    pub(super) fn upstream_stars(&self, query: QueryId, depth: u32) -> Vec<FromId> {
        let Some(Body::Select(select)) = self.hir.queries.get(query).map(|q| &q.body) else {
            return Vec::new();
        };
        if depth > 64 {
            return Vec::new();
        }
        let mut out = Vec::new();
        for item in &select.items {
            if let SelectItem::Star { from, .. } = item {
                for target in self.star_targets(query, *from) {
                    match self.inner_query(target) {
                        Some(inner) => out.extend(self.upstream_stars(inner, depth + 1)),
                        None => out.push(target),
                    }
                }
            }
        }
        out
    }

    /// Whether an in-model query names the column in its output.
    fn outputs(&self, query: QueryId, depth: u32) -> bool {
        if depth > 64 {
            return false;
        }
        let Some(Body::Select(select)) = self.hir.queries.get(query).map(|q| &q.body) else {
            return false;
        };
        select.items.iter().any(|item| match item {
            SelectItem::Expr { alias: Some(a), .. } => self.is(*a),
            SelectItem::Expr { expr, alias: None } => {
                self.bare_col(*expr).is_some_and(|c| self.is(c.name))
            }
            SelectItem::Star { from, .. } => self
                .star_targets(query, *from)
                .into_iter()
                .filter_map(|t| self.inner_query(t))
                .any(|q| self.outputs(q, depth + 1)),
            _ => false,
        })
    }
}
