//! Where a model defines one of its output columns: from the root query
//! down through `select * from cte` and bare `cte.col` pass-throughs to
//! the item that names it. An alias there is renamed (`x as new`); a bare
//! column of an upstream relation gets an alias (`col as new`), so the
//! upstream model is never touched. CTE-internal names above that item
//! change only because the walk carries the new name up to the root.

use cogitator_ir::hir::{Body, QueryId, SelectItem};

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
    ),
    Star(QueryId),
    /// Through `*` over a relation outside the model.
    UpstreamStar,
}

impl Walk<'_> {
    pub(super) fn find_output(&self, query: QueryId) -> Result<Found, String> {
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
                .filter(|p| !matches!(p, Provides::UpstreamStar))
                .collect();
            match explicit.as_slice() {
                [] if !provided.is_empty() => {
                    return Err(format!(
                        "`{column}` comes in through `select *` from an upstream relation; rename it there"
                    ))
                }
                [] => return Err(format!("it outputs no column named `{column}`")),
                [one] => match one {
                    Provides::Alias(item, alias) => {
                        return Ok(Found {
                            query,
                            item: *item,
                            site: self.site(*alias, SqlEdit::Name),
                        })
                    }
                    Provides::Bare(_, Some(inner), _, _) | Provides::Star(inner) => {
                        query = *inner;
                    }
                    Provides::Bare(item, None, name, span) => {
                        let mut site = self.site(*name, SqlEdit::AliasAfter);
                        site.span = *span;
                        return Ok(Found {
                            query,
                            item: *item,
                            site,
                        });
                    }
                    Provides::UpstreamStar => unreachable!("filtered out above"),
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
                    let inner = Self::direct(col.res.as_ref())
                        .and_then(|f| self.inner_query(f))
                        .filter(|q| self.outputs(*q, 0));
                    out.push(Provides::Bare(index, inner, col.name, col.span));
                }
                SelectItem::Star { from, .. } => {
                    for target in self.star_targets(query, *from) {
                        match self.inner_query(target) {
                            Some(inner) if self.outputs(inner, 0) => {
                                out.push(Provides::Star(inner))
                            }
                            Some(_) => {}
                            None => out.push(Provides::UpstreamStar),
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(out)
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
