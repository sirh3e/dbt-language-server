//! Whether an output column is one upstream column passed through
//! unchanged ([`crate::lineage::ColumnLineage::direct`]): a bare column
//! reference, under the name it already had, resolved hop by hop through
//! CTEs, derived tables and stars down to a model or source. Anything
//! else — an expression, a rename, a positional column list, a star
//! `REPLACE`, a set operation, a relation the project does not define —
//! is not, so nothing downstream may claim to be the same column.

use cogitator_ir::hir::{ExprId, ExprKind, FromId, FromKind, Ident, RelTarget, Resolution};
use cogitator_ir::Symbol;

use super::{Row, Walker};

impl Walker<'_> {
    /// A select item `expr [as alias]`.
    pub(super) fn expr_direct(&mut self, expr: ExprId, alias: Option<Ident>) -> bool {
        let hir = self.hir;
        let Some(ExprKind::Col(col)) = hir.exprs.get(expr).map(|e| &e.kind) else {
            return false;
        };
        let Some(col) = hir.cols.get(*col) else {
            return false;
        };
        if alias.is_some_and(|a| a.folded != col.name.folded) {
            return false; // `a as b`: a rename
        }
        let name = col.name.folded;
        match &col.res {
            Some(Resolution::Bound { from, .. })
            | Some(Resolution::Outer { from, .. })
            | Some(Resolution::OpenSchema(from)) => self.item_direct(*from, name),
            _ => false,
        }
    }

    /// Column `name` of the FROM item `from`.
    pub(super) fn item_direct(&mut self, from: FromId, name: Symbol) -> bool {
        let hir = self.hir;
        let Some(item) = hir.from.get(from) else {
            return false;
        };
        match &item.kind {
            FromKind::Base { target, .. } => match target {
                RelTarget::Model { .. } | RelTarget::Source { .. } | RelTarget::Jinja(_) => true,
                RelTarget::Cte(id) => {
                    let Some(cte) = hir.ctes.get(*id) else {
                        return false;
                    };
                    if !cte.columns.is_empty() {
                        return false; // `c(x, y) as (…)` renames by position
                    }
                    let rows = self.rows_of(cte.query);
                    self.rows_direct(&rows, name)
                }
                RelTarget::This | RelTarget::Physical(_) => false,
            },
            FromKind::Derived { q, .. } => {
                let rows = self.rows_of(*q);
                self.rows_direct(&rows, name)
            }
            _ => false,
        }
    }

    /// The one row of `rows` that outputs `name`, passed through.
    fn rows_direct(&mut self, rows: &[Row], name: Symbol) -> bool {
        let mut answering = rows.iter().filter(|row| match (&row.star, row.key) {
            (Some(star), _) => !star.exclude.contains(&name),
            (None, key) => key == Some(name),
        });
        let (Some(row), None) = (answering.next(), answering.next()) else {
            return false; // none, or two candidates: not one column
        };
        match &row.star {
            None => row.direct,
            Some(star) => {
                if star.replace.iter().any(|(n, _)| *n == name) {
                    return false;
                }
                match star.targets.as_slice() {
                    [target] => self.item_direct(*target, name),
                    _ => false, // `*` over a join: which side is not known here
                }
            }
        }
    }
}
