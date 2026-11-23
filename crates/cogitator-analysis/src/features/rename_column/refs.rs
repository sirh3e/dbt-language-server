//! The sites of one document once the renamed FROM items are known:
//! every reference that reads one by the old name, and the checks that
//! refuse what the HIR cannot answer (see [`super::sql`]).

use std::collections::BTreeSet;

use cogitator_ir::hir::{
    Body, ColRefId, ExprKind, FromKind, JoinCond, QueryId, Resolution, SelectItem,
};
use cogitator_parser::{JinjaKind, ParsedDocument, TokenKind};

use super::sql::{SqlEdit, Walk};

impl Walk<'_> {
    /// Every reference that reads a renamed FROM item by the old name.
    pub(super) fn references(&mut self) -> Result<(), String> {
        let column = self.column;
        let own_alias = self.own.filter(|f| f.site.edit == SqlEdit::Name);
        let own_cols = own_alias.map_or_else(Vec::new, |f| self.query_cols(f.query));
        let own_expr = own_alias.and_then(|f| self.item_col(f.query, f.item));
        for (id, col) in self.hir.cols.iter() {
            if !self.is(col.name) || own_expr == Some(id) {
                continue;
            }
            let inside_own = own_cols.contains(&id);
            match col.res.as_ref() {
                Some(Resolution::Candidates(from))
                    if from.iter().any(|f| self.from_hit[f.index()]) =>
                {
                    if from.len() > 1 {
                        return Err(format!(
                            "a bare `{column}` may come from more than one relation; qualify it"
                        ));
                    }
                    self.sites.push(self.site(col.name, SqlEdit::Name));
                }
                Some(Resolution::LateralAlias(item)) if inside_own => {
                    if own_alias.is_some_and(|f| f.item == *item as usize) {
                        self.sites.push(self.site(col.name, SqlEdit::Name));
                    }
                }
                Some(Resolution::Unresolved) if self.from_hit.iter().any(|h| *h) => {
                    return Err(format!("a `{column}` it reads cannot be placed"));
                }
                res => match Self::direct(res) {
                    Some(f) if self.from_hit[f.index()] => {
                        self.sites.push(self.site(col.name, SqlEdit::Name));
                    }
                    Some(_) if inside_own => {
                        return Err(format!(
                            "`{column}` next to its alias may name the alias or an input column"
                        ));
                    }
                    _ => {}
                },
            }
        }
        Ok(())
    }

    /// The column references of a query's own expressions.
    fn query_cols(&self, query: QueryId) -> Vec<ColRefId> {
        let mut out = Vec::new();
        let mut stack = self
            .hir
            .queries
            .get(query)
            .map_or_else(Vec::new, |q| q.exprs());
        while let Some(e) = stack.pop() {
            let Some(expr) = self.hir.exprs.get(e) else {
                continue;
            };
            if let ExprKind::Col(c) = &expr.kind {
                out.push(*c);
            }
            stack.extend(expr.kind.children());
        }
        out
    }

    /// The bare column an item is, by id.
    fn item_col(&self, query: QueryId, item: usize) -> Option<ColRefId> {
        let Some(Body::Select(select)) = self.hir.queries.get(query).map(|q| &q.body) else {
            return None;
        };
        let Some(SelectItem::Expr { expr, .. }) = select.items.get(item) else {
            return None;
        };
        match &self.hir.exprs.get(*expr)?.kind {
            ExprKind::Col(c) => Some(*c),
            _ => None,
        }
    }

    pub(super) fn check_using(&self) -> Result<(), String> {
        for item in self.hir.from.values() {
            if let FromKind::Join {
                l,
                r,
                on: JoinCond::Using(names),
                ..
            } = &item.kind
            {
                let renamed = self.from_hit[l.index()] || self.from_hit[r.index()];
                if renamed && names.iter().any(|n| self.is(*n)) {
                    return Err(format!("a join uses `using ({})`", self.column));
                }
            }
        }
        Ok(())
    }

    /// Other output names of the queries the column flows through.
    pub(super) fn taken(&self) -> Vec<String> {
        let mut out = BTreeSet::new();
        for (id, q) in self.hir.queries.iter() {
            let Body::Select(select) = &q.body else {
                continue;
            };
            if !self.query_hit[id.index()] {
                continue;
            }
            for item in &select.items {
                let SelectItem::Expr { expr, alias } = item else {
                    continue;
                };
                let name = alias.or_else(|| self.bare_col(*expr).map(|c| c.name));
                if let Some(name) = name.filter(|n| !self.is(*n)) {
                    out.insert(self.hir.syms.text(name.folded).to_ascii_lowercase());
                }
            }
        }
        out.into_iter().collect()
    }
}

/// The name as a `'…'` argument of a `{{ … }}` expression: a macro may
/// read the column by name, and nothing here can tell.
pub(super) fn jinja_mentions(parsed: &ParsedDocument, column: &str) -> Result<(), String> {
    let mut inside = false;
    for token in &parsed.tokens {
        match token.kind {
            TokenKind::JinjaOpen(JinjaKind::Expr) => inside = true,
            TokenKind::JinjaClose(JinjaKind::Expr) => inside = false,
            TokenKind::StringLit if inside => {
                let literal = parsed
                    .literal(token)
                    .trim_matches(|c| c == '\'' || c == '"');
                if literal.eq_ignore_ascii_case(column) {
                    return Err(format!(
                        "`{{{{ … '{column}' … }}}}` passes the name to Jinja, which the rename cannot follow"
                    ));
                }
            }
            _ => {}
        }
    }
    Ok(())
}
