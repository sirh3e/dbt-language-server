//! `inlay.types`: each output column's inferred type after its select
//! item in the model's final SELECT (`: bigint`; `≈ ` when the type is
//! not proven, nothing when it is unknown). The types are the schema
//! job's, read off the published snapshot and matched by column name;
//! the positions come from the open document's own HIR, so an unsaved
//! edit moves the hints with the text.

use cogitator_dialects::Dialect;
use cogitator_ir::hir::{Body, SelectItem};
use cogitator_ir::Symbol;
use cogitator_lsp::{inlay_hint_kind, InlayHint, Range};
use cogitator_parser::{LineTable, ParsedDocument};
use cogitator_sql_front::{build_hir, SchemaEnv};

use super::basis::ModelColumnTypes;
use crate::features::plan::diagnostics::span_range;
use crate::lineage::template_input;

/// The type hints of `doc` within `range` (all-zero: the whole document).
pub fn type_inlay_hints(
    doc: &ParsedDocument,
    dialect: &Dialect,
    types: &ModelColumnTypes,
    range: Range,
) -> Vec<InlayHint> {
    let lowered = build_hir(&template_input(doc, dialect), &SchemaEnv::default());
    let Some(hir) = lowered.value.as_ref() else {
        return Vec::new();
    };
    let Some(query) = hir.root.and_then(|r| hir.queries.get(r)) else {
        return Vec::new();
    };
    let Body::Select(select) = &query.body else {
        return Vec::new(); // a set operation's columns are its arms'
    };
    let table = LineTable::new(&doc.text);
    let whole = range == Range::default();
    let starred = select
        .items
        .iter()
        .any(|i| matches!(i, SelectItem::Star { .. }));
    // (name, end offset) per select item: the alias, else the output the
    // item produced (outputs line up with items while no star expands)
    let items: Vec<(Symbol, u32)> = select
        .items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            let SelectItem::Expr { expr, alias } = item else {
                return None;
            };
            let end = hir.exprs.get(*expr)?.span.end;
            match alias {
                Some(a) => Some((a.folded, a.span.end.max(end))),
                None if !starred => Some((query.outputs.get(i)?.name?, end)),
                None => None,
            }
        })
        .collect();
    items
        .into_iter()
        .filter_map(|(name, end)| {
            let name = hir.syms.text(name);
            let typed = types.column(name)?;
            if !super::is_stated(&typed.sql_type) {
                return None;
            }
            let end = cogitator_ir::Span::new(end, end);
            let at = span_range(&doc.text, &table, end)?.start;
            if !whole && (at.line < range.start.line || at.line > range.end.line) {
                return None;
            }
            let spelled = typed.data_type.as_deref().unwrap_or(&typed.description);
            let label = if typed.is_proven() {
                format!(": {spelled}")
            } else {
                format!(": ≈ {spelled}")
            };
            Some(InlayHint {
                position: at,
                label,
                kind: Some(inlay_hint_kind::TYPE),
                padding_left: false,
                padding_right: true,
                tooltip: Some(super::hover::type_line(typed)),
            })
        })
        .collect()
}
