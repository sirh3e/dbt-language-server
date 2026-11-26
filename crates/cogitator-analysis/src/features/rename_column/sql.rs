//! One SQL document's part of a column rename, over its bound HIR.
//!
//! The renamed column starts at a seed: in the model being renamed, the
//! select item that names it ([`super::seed`]); in a consumer, every FROM
//! item that reads an already renamed model. From there it propagates
//! while the name is kept: a FROM item over a CTE or derived table whose
//! output carries the column, a query whose `*` or bare `col` passes it
//! through. Every column reference that reads a renamed FROM item by the
//! old name is a site; an alias that re-names it (`old as other`) keeps
//! the query's output and stops the walk there.
//!
//! Whatever the HIR cannot answer is a refusal, never a guess: Jinja
//! control blocks, SQL that does not parse, a bare name two FROM items
//! may provide, `USING (old)`, a star that excludes or replaces it, or
//! the name inside a `{{ … }}` string, which the HIR cannot see into.

use std::collections::BTreeSet;

use cogitator_dialects::Dialect;
use cogitator_ir::hir::{
    Body, ColRef, FromId, FromKind, Hir, Ident, QueryId, RelTarget, Resolution, SelectItem,
};
use cogitator_ir::{DegradationKind, Lowered, Span};
use cogitator_parser::ParsedDocument;
use cogitator_sql_front::{build_hir, SchemaEnv};

use crate::lineage::{has_control_block, template_input};

/// Where the rename enters a document.
pub(crate) enum Seed<'a> {
    /// The model whose column is renamed: its own output item.
    Own,
    /// A consumer: every FROM item over one of these (renamed) models.
    Upstream(&'a BTreeSet<String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SqlEdit {
    /// Replace the identifier (its quotes kept).
    Name,
    /// A bare `col` of an upstream relation: append ` as new` after it.
    AliasAfter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SqlSite {
    pub span: Span,
    pub edit: SqlEdit,
    /// The identifier's quote character, kept around the new name.
    pub quote: Option<char>,
}

/// What a document contributes.
#[derive(Debug, Default)]
pub(crate) struct DocWalk {
    pub sites: Vec<SqlSite>,
    /// The document's own output carries the column under its old name:
    /// the walk continues from the model it defines.
    pub exposes: bool,
    /// Other output names of the queries the column now flows through,
    /// lowercased, for the collision check.
    pub taken: Vec<String>,
}

/// The document's sites, or the reason it cannot be renamed safely.
pub(crate) fn analyse(
    parsed: &ParsedDocument,
    dialect: &Dialect,
    seed: Seed<'_>,
    column: &str,
) -> Result<DocWalk, String> {
    let lowered = lower(parsed, dialect)?;
    let (hir, root) = bound(&lowered)?;
    let mut walk = Walk::new(hir, &parsed.text, column);
    match seed {
        Seed::Own => {
            let found = walk.find_output(root)?;
            walk.query_hit[found.query.index()] = true;
            walk.sites.push(found.site);
            walk.own = Some(found);
        }
        Seed::Upstream(models) => {
            for (id, item) in hir.from.iter() {
                if let FromKind::Base {
                    target: RelTarget::Model { name, .. },
                    ..
                } = &item.kind
                {
                    if models.contains(hir.syms.text(*name)) {
                        walk.from_hit[id.index()] = true;
                    }
                }
            }
        }
    }
    walk.propagate()?;
    if !walk.from_hit.iter().any(|h| *h) && walk.own.is_none() {
        return Ok(DocWalk::default());
    }
    walk.references()?;
    walk.check_using()?;
    super::refs::jinja_mentions(parsed, column)?;
    Ok(DocWalk {
        exposes: walk.query_hit[root.index()],
        taken: walk.taken(),
        sites: walk.sites,
    })
}

/// The document's bound HIR, or why a rename cannot trust it.
pub(super) fn lower(parsed: &ParsedDocument, dialect: &Dialect) -> Result<Lowered<Hir>, String> {
    if has_control_block(parsed) {
        return Err("its Jinja control blocks hide which columns it reads".into());
    }
    let lowered = build_hir(&template_input(parsed, dialect), &SchemaEnv::default());
    let incomplete = lowered.degraded.iter().any(|d| {
        matches!(
            d.kind,
            DegradationKind::ParseHole | DegradationKind::CapExceeded
        )
    });
    match (&lowered.value, incomplete) {
        (Some(_), false) => Ok(lowered),
        _ => Err("its SQL does not parse completely".into()),
    }
}

/// The HIR of a [`lower`]ed document and its root query.
pub(super) fn bound(lowered: &Lowered<Hir>) -> Result<(&Hir, QueryId), String> {
    let hir = lowered
        .value
        .as_ref()
        .ok_or("its SQL does not parse completely")?;
    Ok((hir, hir.root.ok_or("its SQL has no query")?))
}

/// The item that defines the column in the model being renamed.
#[derive(Debug, Clone, Copy)]
pub(super) struct Found {
    pub query: QueryId,
    pub item: usize,
    pub site: SqlSite,
}

pub(super) struct Walk<'h> {
    pub hir: &'h Hir,
    pub text: &'h str,
    pub column: &'h str,
    pub from_hit: Vec<bool>,
    pub query_hit: Vec<bool>,
    pub sites: Vec<SqlSite>,
    pub own: Option<Found>,
}

impl<'h> Walk<'h> {
    pub(super) fn new(hir: &'h Hir, text: &'h str, column: &'h str) -> Self {
        Walk {
            hir,
            text,
            column,
            from_hit: vec![false; hir.from.len()],
            query_hit: vec![false; hir.queries.len()],
            sites: Vec::new(),
            own: None,
        }
    }

    pub(super) fn is(&self, ident: Ident) -> bool {
        self.hir
            .syms
            .text(ident.folded)
            .eq_ignore_ascii_case(self.column)
    }

    pub(super) fn site(&self, ident: Ident, edit: SqlEdit) -> SqlSite {
        let quote = ident
            .quoted
            .then(|| self.text.get(ident.span.start as usize..))
            .flatten()
            .and_then(|s| s.chars().next());
        SqlSite {
            span: ident.span,
            edit,
            quote,
        }
    }

    /// The FROM item a reference reads directly, when the binder knows.
    pub(super) fn direct(res: Option<&Resolution>) -> Option<FromId> {
        match res? {
            Resolution::Bound { from, .. }
            | Resolution::Outer { from, .. }
            | Resolution::OpenSchema(from) => Some(*from),
            _ => None,
        }
    }

    /// The query a FROM item reads inside this document, if any.
    pub(super) fn inner_query(&self, from: FromId) -> Option<QueryId> {
        match &self.hir.from.get(from)?.kind {
            FromKind::Base {
                target: RelTarget::Cte(cte),
                ..
            } => {
                let cte = self.hir.ctes.get(*cte)?;
                cte.columns.is_empty().then_some(cte.query)
            }
            FromKind::Derived { q, .. } => Some(*q),
            _ => None,
        }
    }

    /// The FROM items a star item expands.
    pub(super) fn star_targets(&self, query: QueryId, from: Option<FromId>) -> Vec<FromId> {
        match from {
            Some(from) => vec![from],
            None => self
                .hir
                .queries
                .get(query)
                .and_then(|q| self.hir.scopes.get(q.scope))
                .map(|s| s.from.clone()) // boundary: a short per-select list
                .unwrap_or_default(),
        }
    }

    /// Mark FROM items and queries the renamed column flows through,
    /// until nothing changes.
    fn propagate(&mut self) -> Result<(), String> {
        loop {
            let mut changed = false;
            for id in self.hir.from.ids() {
                let hit = self
                    .inner_query(id)
                    .is_some_and(|q| self.query_hit[q.index()]);
                if hit && !self.from_hit[id.index()] {
                    self.from_hit[id.index()] = true;
                    changed = true;
                }
            }
            for id in self.hir.queries.ids() {
                if !self.query_hit[id.index()] && self.passes_through(id)? {
                    self.query_hit[id.index()] = true;
                    changed = true;
                }
            }
            if !changed {
                return Ok(());
            }
        }
    }

    /// Whether `query` outputs a renamed input under the old name.
    fn passes_through(&self, query: QueryId) -> Result<bool, String> {
        let Some(q) = self.hir.queries.get(query) else {
            return Ok(false);
        };
        let column = self.column;
        match &q.body {
            Body::Select(select) => {
                for item in &select.items {
                    match item {
                        SelectItem::Star {
                            from,
                            exclude,
                            replace,
                            ..
                        } => {
                            let targets = self.star_targets(query, *from);
                            if !targets.iter().any(|t| self.from_hit[t.index()]) {
                                continue;
                            }
                            if exclude.iter().any(|e| self.is(*e))
                                || replace.iter().any(|(_, n)| self.is(*n))
                            {
                                return Err(format!(
                                    "a `* exclude`/`replace` names `{column}`; edit it by hand"
                                ));
                            }
                            return Ok(true);
                        }
                        SelectItem::Expr { expr, alias: None } if self.reads_hit(*expr) => {
                            return Ok(true);
                        }
                        _ => {}
                    }
                }
                Ok(false)
            }
            Body::SetOp { by_name, l, r, .. } => {
                let (l, r) = (self.query_hit[l.index()], self.query_hit[r.index()]);
                if *by_name && (l || r) {
                    return Err(format!("`{column}` feeds a `union by name`"));
                }
                Ok(l)
            }
            Body::Values(_) | Body::Hole(_) => Ok(false),
        }
    }

    /// A bare column reading a renamed FROM item by the old name.
    fn reads_hit(&self, expr: cogitator_ir::hir::ExprId) -> bool {
        self.bare_col(expr).is_some_and(|c| {
            self.is(c.name)
                && Self::direct(c.res.as_ref()).is_some_and(|f| self.from_hit[f.index()])
        })
    }

    /// The column reference an expression is, when it is nothing else.
    pub(super) fn bare_col(&self, expr: cogitator_ir::hir::ExprId) -> Option<&'h ColRef> {
        match &self.hir.exprs.get(expr)?.kind {
            cogitator_ir::hir::ExprKind::Col(c) => self.hir.cols.get(*c),
            _ => None,
        }
    }
}
