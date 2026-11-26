//! Rows: what each query outputs, one [`Row`] per select item.

use cogitator_ir::hir::{Body, ExprId, ExprKind, FromId, Ident, QueryId, SelectItem};

use super::direct::Hop;
use super::{unique, Ctx, Row, Star, Walker};
use crate::lineage::SourceRef;

impl Walker<'_> {
    pub(super) fn build_rows(&mut self, id: QueryId) -> Vec<Row> {
        let Some(query) = self.hir.queries.get(id) else {
            return Vec::new();
        };
        match &query.body {
            Body::Select(select) => {
                let ctx = Ctx {
                    scope: query.scope,
                    select: Some(select),
                };
                let mut rows = Vec::with_capacity(select.items.len());
                for item in &select.items {
                    let end = item_end(self.hir, item);
                    let made = match self.star_columns(item, ctx) {
                        Some(columns) => columns,
                        None => self.item_row(item, ctx, id).into_iter().collect(),
                    };
                    // a star's column keeps where an in-file CTE made it
                    rows.extend(made.into_iter().map(|row| Row {
                        end: row.end.or(end),
                        ..row
                    }));
                }
                rows
            }
            Body::SetOp { by_name, l, r, .. } => {
                let (left, right) = (self.rows_of(*l), self.rows_of(*r));
                if *by_name {
                    merge_by_name(&left, &right)
                } else {
                    merge_by_position(&left, &right)
                }
            }
            Body::Values(rows) => self.values_rows(rows, id),
            Body::Hole(_) => Vec::new(),
        }
    }

    /// One row per known column of a star. An `Open` input (a CTE with a
    /// `{{ macro() }}` select item, a model with columns beyond those
    /// known) adds a `*` row after its columns, excluding them, for what
    /// is not known. `None` when an input knows no column at all, or the
    /// item is no star: the caller then keeps the `*` row, so nothing here
    /// guesses.
    fn star_columns(&mut self, item: &SelectItem, ctx: Ctx<'_>) -> Option<Vec<Row>> {
        let SelectItem::Star {
            from,
            exclude,
            replace,
            ..
        } = item
        else {
            return None;
        };
        if !self.expand_stars {
            return None;
        }
        let targets = match from {
            Some(from) => vec![*from],
            None => self.scope_leaves(ctx),
        };
        let hir = self.hir;
        let known = |t: &FromId| hir.from.get(*t).is_some_and(|f| !f.output.is_empty());
        if targets.is_empty() || !targets.iter().all(known) {
            return None;
        }
        let mut rows = Vec::new();
        for target in targets {
            rows.extend(self.known_rows(target, exclude, replace, ctx));
            if hir.from.get(target).is_some_and(|f| f.open) {
                rows.push(self.residual_star(target, exclude, replace, ctx));
            }
        }
        Some(rows)
    }

    /// `None` only for a `t.*` whose qualifier names nothing in scope.
    fn item_row(&mut self, item: &SelectItem, ctx: Ctx<'_>, query: QueryId) -> Option<Row> {
        match item {
            SelectItem::Expr { expr, alias } => Some(self.expr_row(*expr, *alias, ctx, query)),
            SelectItem::Star {
                from,
                qualifier,
                exclude,
                replace,
                ..
            } => {
                let targets = match (from, qualifier) {
                    (Some(from), _) => vec![*from],
                    (None, Some(_)) => return None,
                    (None, None) => self.scope_leaves(ctx),
                };
                let label = match qualifier {
                    Some(q) => format!("{}.*", self.surface(*q)),
                    None => "*".to_string(),
                };
                let star = Star {
                    exclude: exclude.iter().map(|i| i.folded).collect(),
                    replace: replace
                        .iter()
                        .map(|(e, name)| (name.folded, self.expr_sources(*e, ctx)))
                        .collect(),
                    targets: targets.clone(), // boundary: a short per-select list
                };
                Some(Row {
                    key: None,
                    label: Some(label),
                    star: Some(star),
                    sources: self.star_sources(&targets, ctx),
                    ..Row::default()
                })
            }
            // A placeholder item keeps positions aligned for column lists
            SelectItem::Jinja(_) | SelectItem::Hole(_) => Some(Row::default()),
        }
    }

    pub(super) fn star_sources(&mut self, targets: &[FromId], ctx: Ctx<'_>) -> Vec<SourceRef> {
        let mut out = Vec::new();
        for target in targets {
            out.extend(self.star_of_item(*target, ctx));
        }
        unique(out)
    }

    fn expr_row(
        &mut self,
        expr: ExprId,
        alias: Option<Ident>,
        ctx: Ctx<'_>,
        query: QueryId,
    ) -> Row {
        let sources = self.expr_sources(expr, ctx);
        let direct = self.expr_direct(expr, alias, Hop::Unchanged);
        let passes = self.expr_direct(expr, alias, Hop::Renamed);
        let named = match alias {
            Some(alias) => Some((alias.folded, self.surface(alias))),
            None => self.implicit_name(expr, query),
        };
        match named {
            Some((key, label)) => Row {
                key: Some(key),
                label: Some(label),
                star: None,
                sources,
                direct,
                passes,
                end: None,
            },
            None => Row {
                sources,
                ..Row::default()
            },
        }
    }

    /// `col`, `t.col` and the struct path `s.f` name their output.
    fn implicit_name(
        &self,
        expr: ExprId,
        query: QueryId,
    ) -> Option<(cogitator_ir::Symbol, String)> {
        let node = self.hir.exprs.get(expr)?;
        if let ExprKind::Col(col) = &node.kind {
            let col = self.hir.cols.get(*col)?;
            return Some((col.name.folded, self.surface(col.name)));
        }
        let output = self
            .hir
            .queries
            .get(query)?
            .outputs
            .iter()
            .find(|o| o.expr == Some(expr))?;
        let key = output.name?;
        let written = self
            .text
            .get(output.span.start as usize..output.span.end as usize)?;
        let last = written.rsplit('.').next()?.trim_matches('"');
        Some((key, last.to_string())) // boundary: reports own their names
    }

    /// VALUES has no names; each position still has its expressions' sources.
    fn values_rows(&mut self, rows: &[Vec<ExprId>], query: QueryId) -> Vec<Row> {
        let scope = self.hir.queries.get(query).map(|q| q.scope);
        let Some(scope) = scope else {
            return Vec::new();
        };
        let ctx = Ctx {
            scope,
            select: None,
        };
        let width = rows.first().map_or(0, Vec::len);
        (0..width)
            .map(|i| {
                let mut sources = Vec::new();
                for row in rows {
                    if let Some(expr) = row.get(i) {
                        sources.extend(self.expr_sources(*expr, ctx));
                    }
                }
                Row {
                    sources: unique(sources),
                    ..Row::default()
                }
            })
            .collect()
    }
}

/// Where a select item ends: past its alias when it has one.
fn item_end(hir: &cogitator_ir::hir::Hir, item: &SelectItem) -> Option<u32> {
    match item {
        SelectItem::Expr { expr, alias } => {
            let expr = hir.exprs.get(*expr).map(|e| e.span.end);
            expr.max(alias.map(|a| a.span.end))
        }
        SelectItem::Star { span, .. } => Some(span.end),
        SelectItem::Jinja(_) | SelectItem::Hole(_) => None,
    }
}

/// A set operation outputs by position; the left arm names the columns.
fn merge_by_position(left: &[Row], right: &[Row]) -> Vec<Row> {
    let width = left.len().max(right.len());
    (0..width)
        .map(|i| match (left.get(i), right.get(i)) {
            (Some(l), Some(r)) => merged(l, r),
            (Some(row), None) | (None, Some(row)) => row.clone(), // boundary: per-query rows
            (None, None) => Row::default(),
        })
        .collect()
}

/// `UNION BY NAME`: matching names combine, the rest are appended.
fn merge_by_name(left: &[Row], right: &[Row]) -> Vec<Row> {
    let mut out: Vec<Row> = left.to_vec(); // boundary: per-query rows
    for row in right {
        let same = out.iter().position(|o| o.key.is_some() && o.key == row.key);
        match same {
            Some(i) => out[i] = merged(&out[i], row),
            None => out.push(row.clone()), // boundary: per-query rows
        }
    }
    out
}

fn merged(l: &Row, r: &Row) -> Row {
    let named = if l.label.is_some() { l } else { r };
    let mut sources = l.sources.clone(); // boundary: per-query rows
    sources.extend(r.sources.iter().cloned());
    let star = match (&l.star, &r.star) {
        (Some(a), Some(b)) => Some(merged_stars(a, b)),
        (Some(s), None) | (None, Some(s)) => Some(s.clone()),
        (None, None) => None,
    };
    // a set operation mixes arms: no column of it is one upstream column
    Row {
        key: named.key,
        label: named.label.clone(),
        star,
        sources: unique(sources),
        end: l.end.or(r.end),
        ..Row::default()
    }
}

/// A column an arm still provides is not excluded from the union; a
/// replacement in either arm feeds it.
fn merged_stars(a: &Star, b: &Star) -> Star {
    let mut replace = a.replace.clone(); // boundary: per-query rows
    replace.extend(b.replace.iter().cloned());
    Star {
        exclude: a
            .exclude
            .iter()
            .filter(|name| b.exclude.contains(name))
            .copied()
            .collect(),
        replace,
        targets: Vec::new(),
    }
}
