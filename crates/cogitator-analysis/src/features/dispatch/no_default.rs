//! `dispatch-no-default`: an `adapter.dispatch('x', …)` that finds
//! neither `default__x` nor an implementation for the active adapter in
//! its search order fails at run time on this target.

use cogitator_error::guidance::Guidance;
use cogitator_lsp::{diagnostic_severity, Diagnostic, Range};
use cogitator_parser::macro_body::dispatch_sites;
use cogitator_parser::LineTable;

use super::{active_implementation, search_packages, DBT_NAMESPACE};
use crate::features::{DbtContext, Document};

pub const CODE: &str = "dispatch-no-default";

/// One warning per dispatch whose namespace is scanned (the root project
/// or an installed package) and that resolves nowhere. Without a
/// namespace, only names some scanned `<prefix>__x` implements.
pub fn check(ctx: &DbtContext, doc: &Document) -> Vec<Diagnostic> {
    let table = LineTable::new(&doc.text);
    dispatch_sites(&doc.text)
        .into_iter()
        .filter(|site| match site.call.namespace.as_deref() {
            // dbt's own namespace and unknown packages are not ours to judge
            Some(ns) => ns != DBT_NAMESPACE && ctx.macro_detail_map.contains_key(ns),
            // no namespace: dbt's own macros may answer it, so only
            // judge a name the project implements somewhere itself
            None => !super::implementations(ctx, &site.call.name).is_empty(),
        })
        .filter(|site| {
            let configured = site
                .call
                .namespace
                .as_deref()
                .is_some_and(|ns| ctx.project.dispatch.iter().any(|d| d.macro_namespace == ns));
            // a configured search order that only names dbt is dbt's
            let order = search_packages(ctx, site.call.namespace.as_deref());
            !(configured && order.iter().all(|p| p == DBT_NAMESPACE))
        })
        .filter(|site| active_implementation(ctx, &site.call).is_none())
        .map(|site| {
            let (sl, sc) = table.line_col(site.start);
            let (el, ec) = table.line_col(site.end);
            let range = Range {
                start: cogitator_lsp::Position {
                    line: sl,
                    character: sc,
                },
                end: cogitator_lsp::Position {
                    line: el,
                    character: ec,
                },
            };
            let adapter = match ctx.dialect.as_str() {
                "" => "the active adapter",
                a => a,
            };
            let name = &site.call.name;
            Diagnostic::plain(
                range,
                Guidance::new(format!(
                    "adapter.dispatch('{name}') finds neither default__{name} nor an \
                     implementation for {adapter}"
                ))
                .fix(format!(
                    "define default__{name} (or {adapter}__{name}) in a macro file."
                ))
                .to_diagnostic(),
                diagnostic_severity::WARNING,
                CODE,
                "cogitator",
            )
        })
        .collect()
}
