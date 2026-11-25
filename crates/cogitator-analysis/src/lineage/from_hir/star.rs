//! A star's known columns, and the `*` row standing for the rest of an
//! open input (a CTE with a `{{ macro() }}` select item).

use cogitator_ir::hir::{ExprId, FromId, Ident};

use super::lookup::Wanted;
use super::{unique, Ctx, Row, Star, Walker};

impl Walker<'_> {
    /// The rows of the columns `target` is known to output.
    pub(super) fn known_rows(
        &mut self,
        target: FromId,
        exclude: &[Ident],
        replace: &[(ExprId, Ident)],
        ctx: Ctx<'_>,
    ) -> Vec<Row> {
        let hir = self.hir;
        let Some(item) = hir.from.get(target) else {
            return Vec::new();
        };
        let mut rows = Vec::new();
        for (position, col) in item.output.iter().enumerate() {
            if exclude.iter().any(|e| e.folded == col.name) {
                continue;
            }
            let replaced = replace.iter().find(|(_, name)| name.folded == col.name);
            let label = hir.syms.text(col.name).to_string(); // boundary: reports own names
            let wanted = Wanted {
                key: Some(col.name),
                label: &label,
                bound: u32::try_from(position).ok(),
            };
            let (sources, direct) = match replaced {
                Some((expr, _)) => (self.expr_sources(*expr, ctx), false),
                None => (
                    self.item_sources(target, wanted, ctx),
                    self.item_direct(target, col.name),
                ),
            };
            rows.push(Row {
                key: Some(col.name),
                label: Some(label),
                star: None,
                sources: unique(sources),
                direct,
            });
        }
        rows
    }

    /// What an open `target` outputs beyond its known columns: a `*` row
    /// that answers no name already listed.
    pub(super) fn residual_star(
        &mut self,
        target: FromId,
        exclude: &[Ident],
        replace: &[(ExprId, Ident)],
        ctx: Ctx<'_>,
    ) -> Row {
        let hir = self.hir;
        let mut excluded: Vec<cogitator_ir::Symbol> = exclude.iter().map(|e| e.folded).collect();
        if let Some(item) = hir.from.get(target) {
            excluded.extend(item.output.iter().map(|c| c.name));
        }
        let star = Star {
            exclude: excluded,
            replace: replace
                .iter()
                .map(|(e, name)| (name.folded, self.expr_sources(*e, ctx)))
                .collect(),
            targets: vec![target],
        };
        Row {
            key: None,
            label: Some("*".to_string()),
            star: Some(star),
            sources: self.star_sources(&[target], ctx),
            direct: false,
        }
    }
}
