//! AST to HIR: name resolution happens during the walk, because a scope
//! is only meaningful while its FROM items are in hand. The AST is walked
//! by hand (sqlparser's visitor is unusable at this version).

use std::collections::HashMap;

use cogitator_ir::common::{HoleKind, ModelConfig};
use cogitator_ir::hir::{
    node_count, Body, CteId, Expr, ExprId, ExprKind, From, FromCol, FromId, FromKind, Hir, Ident,
    JinjaId, JinjaKind, JinjaNode, JinjaRole, OutputCol, Query, QueryId, Scope, ScopeId, ScopeKind,
};
use cogitator_ir::origin::{SourceOrigin, TemplateVariant};
use cogitator_ir::{Degradation, DegradationKind, Interner, Span, Symbol};
use sqlparser::ast;

use crate::caps::MAX_NODES;
use crate::config::apply_config;
use crate::dialect::DialectRules;
use crate::mask::{Region, RegionKind, Scanned};
use crate::parse::{Outcome, Parsed, Part, Parts};
use crate::segment::Positions;
use crate::{HirInput, SchemaEnv};

mod binder;
mod call;
mod expr;
mod from;
mod query;
mod select;
mod star;

use binder::{AliasMode, Frame};

/// Above this width a FROM item gets a name index; below, a scan is faster.
const INDEXED_WIDTH: usize = 64;

/// One lowering of one template. `words` maps placeholder bytes to regions.
pub(crate) struct Lowerer<'a> {
    pub(crate) hir: Hir,
    pos: &'a Positions,
    regions: &'a [Region],
    /// Sorted by start: `(placeholder word, region index)`.
    words: Vec<(Span, usize)>,
    consumed: Vec<bool>,
    env: &'a SchemaEnv,
    rules: DialectRules,
    frames: Vec<Frame>,
    /// CTEs in scope, innermost last.
    visible: Vec<(Symbol, CteId)>,
    /// Per `FromId`: the names a qualifier may use, outermost first.
    from_names: Vec<Vec<Symbol>>,
    /// Per `FromId`: the output name index for wide items.
    from_index: Vec<Option<HashMap<Symbol, u32>>>,
    /// Per `QueryId`: true when the output list may be incomplete.
    query_open: Vec<bool>,
    root_scope: ScopeId,
    /// Lambda parameters in scope; their names are not columns.
    lambda: Vec<Symbol>,
    mode: AliasMode,
    depth: usize,
    /// Output columns charged against the node cap.
    extra_nodes: usize,
    cap_reported: bool,
    depth_reported: bool,
    degraded: Vec<Degradation>,
}

/// Lowers a parsed statement into a bound HIR plus its degradations.
pub(crate) fn lower(
    parsed: Parsed,
    scanned: &Scanned,
    pos: &Positions,
    input: &HirInput<'_>,
    env: &SchemaEnv,
) -> (Hir, Vec<Degradation>) {
    let origin = variant_origin(input, scanned.variant);
    let rules = DialectRules::for_dialect(input.dialect);
    let mut hir = Hir::new(origin);
    let doc_span = Span::new(0, input.text.len() as u32);
    let root_scope = hir.scopes.alloc(Scope {
        span: doc_span,
        parent: None,
        kind: ScopeKind::Root,
        from: Vec::new(),
    });
    hir.config = merged_config(input, scanned, &mut hir.syms, &rules);
    alloc_regions(&mut hir, &scanned.regions);
    let words = placeholder_index(&scanned.regions);
    let mut lowerer = Lowerer {
        consumed: vec![false; scanned.regions.len()],
        words,
        hir,
        pos,
        regions: &scanned.regions,
        env,
        rules,
        frames: Vec::new(),
        visible: Vec::new(),
        from_names: Vec::new(),
        from_index: Vec::new(),
        query_open: Vec::new(),
        root_scope,
        lambda: Vec::new(),
        mode: AliasMode::Off,
        depth: 0,
        extra_nodes: 0,
        cap_reported: false,
        depth_reported: false,
        degraded: parsed.degraded,
    };
    let root = lowerer.outcome(&parsed.outcome, doc_span);
    lowerer.hir.root = Some(root);
    (lowerer.hir, lowerer.degraded)
}

fn variant_origin(input: &HirInput<'_>, variant: TemplateVariant) -> SourceOrigin {
    match &input.origin {
        SourceOrigin::Template { .. } => SourceOrigin::Template { variant },
        // boundary: the origin is owned by the plan (08 §1.5)
        other => other.clone(),
    }
}

fn merged_config(
    input: &HirInput<'_>,
    scanned: &Scanned,
    syms: &mut Interner,
    rules: &DialectRules,
) -> ModelConfig {
    // boundary: the plan owns its config (08 §1.5)
    let mut config = input.config.clone();
    for kwargs in &scanned.configs {
        apply_config(&mut config, kwargs, syms, rules);
    }
    config
}

fn alloc_regions(hir: &mut Hir, regions: &[Region]) {
    for region in regions {
        let kind = region_kind(&region.kind, &mut hir.syms);
        hir.jinja.alloc(JinjaNode {
            span: region.span,
            kind,
            role: region.initial_role(),
        });
    }
}

fn region_kind(kind: &RegionKind, syms: &mut Interner) -> JinjaKind {
    match kind {
        RegionKind::Ref => JinjaKind::Ref,
        RegionKind::Source => JinjaKind::Source,
        RegionKind::This => JinjaKind::This,
        RegionKind::Var => JinjaKind::Var,
        RegionKind::Macro(name) => JinjaKind::Macro(syms.intern(name)),
        RegionKind::IsIncremental => JinjaKind::IsIncremental,
        RegionKind::Config => JinjaKind::Config,
        RegionKind::Expression => JinjaKind::Expression,
        RegionKind::Statement => JinjaKind::Statement,
        RegionKind::Comment => JinjaKind::Comment,
    }
}

fn placeholder_index(regions: &[Region]) -> Vec<(Span, usize)> {
    let mut words: Vec<(Span, usize)> = regions
        .iter()
        .enumerate()
        .filter_map(|(i, r)| r.word.map(|w| (w, i)))
        .collect();
    words.sort_by_key(|(w, i)| (w.start, *i));
    words
}

impl Lowerer<'_> {
    fn outcome(&mut self, outcome: &Outcome, doc_span: Span) -> QueryId {
        match outcome {
            Outcome::Whole(query) => self.query(query, ScopeKind::Query, false),
            Outcome::Parts(parts) => self.parts(parts),
            Outcome::Hole(kind, span) => {
                let span = if span.is_empty() { doc_span } else { *span };
                self.hole_query(*kind, span, Vec::new())
            }
        }
    }

    /// The root of a statement that only parsed in pieces.
    fn parts(&mut self, parts: &Parts) -> QueryId {
        let scope = self.alloc_scope(ScopeKind::Query, self.root_scope, Span::default());
        self.frames.push(Frame::new(scope, false));
        let mark = self.visible.len();
        let ctes = self.cte_parts(parts);
        let root = match &parts.body {
            Part::Parsed(query) => {
                let id = self.query(query, ScopeKind::Query, false);
                self.prepend_ctes(id, ctes);
                id
            }
            Part::Hole(kind, span) => self.hole_query(*kind, *span, ctes),
        };
        self.visible.truncate(mark);
        self.frames.pop();
        root
    }

    pub(super) fn prepend_ctes(&mut self, id: QueryId, ctes: Vec<CteId>) {
        if let Some(query) = self.hir.queries.get_mut(id) {
            let own = std::mem::replace(&mut query.ctes, ctes);
            query.ctes.extend(own);
        }
    }

    pub(super) fn hole_query(&mut self, kind: HoleKind, span: Span, ctes: Vec<CteId>) -> QueryId {
        let parent = self.current_scope();
        let scope = self.alloc_scope(ScopeKind::Query, parent, span);
        self.alloc_query(
            Query {
                span,
                scope,
                ctes,
                body: Body::Hole(kind),
                order_by: Vec::new(),
                limit: None,
                outputs: Vec::new(),
            },
            true,
        )
    }

    pub(super) fn current_scope(&self) -> ScopeId {
        self.frames.last().map_or(self.root_scope, |f| f.scope)
    }

    pub(super) fn alloc_scope(&mut self, kind: ScopeKind, parent: ScopeId, span: Span) -> ScopeId {
        self.hir.scopes.alloc(Scope {
            span,
            parent: Some(parent),
            kind,
            from: Vec::new(),
        })
    }

    pub(super) fn alloc_query(&mut self, query: Query, open: bool) -> QueryId {
        let id = self.hir.queries.alloc(query);
        self.query_open.push(open);
        id
    }

    pub(super) fn alloc_from(
        &mut self,
        span: Span,
        kind: FromKind,
        output: Vec<FromCol>,
        open: bool,
        names: Vec<Symbol>,
    ) -> FromId {
        self.charge(output.len());
        let index = (output.len() > INDEXED_WIDTH).then(|| name_index(&output));
        let id = self.hir.from.alloc(From {
            span,
            kind,
            output,
            open,
        });
        self.from_names.push(names);
        self.from_index.push(index);
        id
    }

    pub(super) fn alloc_expr(&mut self, span: Span, kind: ExprKind) -> ExprId {
        self.hir.exprs.alloc(Expr { span, kind })
    }

    /// Charges `n` non-arena nodes (output columns) to the node cap.
    pub(super) fn charge(&mut self, n: usize) {
        self.extra_nodes = self.extra_nodes.saturating_add(n);
    }

    /// True once the plan is at the node cap; reported once.
    pub(super) fn over_budget(&mut self) -> bool {
        let over = node_count(&self.hir) + self.extra_nodes > MAX_NODES;
        if over && !self.cap_reported {
            self.cap_reported = true;
            self.degraded.push(Degradation::new(
                DegradationKind::CapExceeded,
                format!("the plan exceeds {MAX_NODES} nodes; the rest is holes"),
            ));
        }
        over
    }

    pub(super) fn degrade(&mut self, kind: DegradationKind, span: Span, detail: impl Into<String>) {
        let degradation = Degradation::new(kind, detail);
        self.degraded.push(if span.is_empty() {
            degradation
        } else {
            degradation.at(span)
        });
    }

    /// Exact byte span of a token-derived sqlparser span.
    pub(super) fn sql_span(&self, span: sqlparser::tokenizer::Span) -> Option<Span> {
        self.pos.span(span)
    }

    pub(super) fn ident_span(&self, ident: &ast::Ident) -> Span {
        self.sql_span(ident.span).unwrap_or_default()
    }

    pub(super) fn path_span(&self, path: &[ast::Ident]) -> Span {
        let span_of = |i: &ast::Ident| self.sql_span(i.span);
        match (
            path.first().and_then(span_of),
            path.last().and_then(span_of),
        ) {
            (Some(first), Some(last)) => Span::new(first.start, last.end),
            (one, _) => one.unwrap_or_default(),
        }
    }

    pub(super) fn ident(&mut self, ident: &ast::Ident) -> Ident {
        let folded = self.rules.fold_ident(ident);
        let folded = self.hir.syms.intern(&folded);
        Ident {
            span: self.ident_span(ident),
            folded,
            quoted: ident.quote_style.is_some(),
        }
    }

    pub(super) fn sym_for(&mut self, ident: &ast::Ident) -> Symbol {
        let folded = self.rules.fold_ident(ident);
        self.hir.syms.intern(&folded)
    }

    pub(super) fn intern(&mut self, text: &str) -> Symbol {
        self.hir.syms.intern(text)
    }

    /// `alias` unless it is a placeholder, whose text is not a name.
    pub(super) fn alias_ident(&mut self, ident: &ast::Ident) -> Option<Ident> {
        let span = self.ident_span(ident);
        if self.region_at(span).is_some() {
            return None;
        }
        Some(self.ident(ident))
    }

    /// The region whose placeholder overlaps `span`.
    pub(super) fn region_at(&self, span: Span) -> Option<usize> {
        let i = self.words.partition_point(|(w, _)| w.end <= span.start);
        self.words
            .get(i)
            .filter(|(w, _)| w.start < span.end)
            .map(|(_, r)| *r)
    }

    /// The region overlapping any ident in `path`.
    pub(super) fn region_in_path(&self, path: &[ast::Ident]) -> Option<usize> {
        path.iter()
            .find_map(|ident| self.region_at(self.ident_span(ident)))
    }

    /// Claims region `index` for one consumer, setting its role. `None`
    /// when it was already taken, which the caller treats as a hole.
    pub(super) fn take_region(&mut self, index: usize, role: JinjaRole) -> Option<JinjaId> {
        let slot = self.consumed.get_mut(index)?;
        if std::mem::replace(slot, true) {
            return None;
        }
        let id = JinjaId::from_raw(index as u32);
        if let Some(node) = self.hir.jinja.get_mut(id) {
            node.role = role;
        }
        Some(id)
    }

    pub(super) fn region(&self, index: usize) -> Option<&Region> {
        self.regions.get(index)
    }

    /// The region's span, for a node standing in for it.
    pub(super) fn region_span(&self, index: usize) -> Span {
        self.region(index).map_or(Span::default(), |r| r.span)
    }

    pub(super) fn output_col(span: Span, name: Option<Symbol>, expr: Option<ExprId>) -> OutputCol {
        OutputCol {
            span,
            name,
            expr,
            ty: cogitator_ir::ColType::UNKNOWN,
        }
    }

    /// Spans known and joined; approximate by construction (see
    /// `ExprKind::span_is_exact`).
    pub(super) fn union_spans(&self, children: &[ExprId]) -> Span {
        let mut out: Option<Span> = None;
        for child in children {
            if let Some(expr) = self.hir.exprs.get(*child) {
                if !expr.span.is_empty() {
                    out = Some(out.map_or(expr.span, |s: Span| union(s, expr.span)));
                }
            }
        }
        out.unwrap_or_default()
    }
}

pub(super) fn union(a: Span, b: Span) -> Span {
    Span::new(a.start.min(b.start), a.end.max(b.end))
}

fn name_index(output: &[FromCol]) -> HashMap<Symbol, u32> {
    let mut index = HashMap::with_capacity(output.len());
    for (i, col) in output.iter().enumerate() {
        index.entry(col.name).or_insert(i as u32);
    }
    index
}
