//! `dbt.search` (docs/search.md): a structural query over every model's
//! SQL, read through the front end's bound HIR of the live template —
//! the joins, `*`s, relations and column references that ast-grep's
//! dbt grammar cannot see (it treats SQL as text). Pure: file text
//! comes through the `read` closure. A model whose HIR cannot be built
//! (Jinja control blocks, SQL that does not parse) is listed as not
//! searched, with why — never dropped silently.

mod eval;
pub mod expr;
pub mod query;

use std::collections::BTreeMap;
use std::path::Path;

use cogitator_error::guidance::Guidance;
use cogitator_ir::DegradationKind;
use cogitator_lsp::Range;
use cogitator_parser::LineTable;
use cogitator_sql_front::{build_hir, SchemaEnv};
use serde::Serialize;

use super::plan::diagnostics::span_range;
use super::DbtContext;
use crate::lineage::{has_control_block, template_input};

/// The most matches one answer carries.
pub const MAX_MATCHES: usize = 500;

/// One site, shaped like a `dbt.astGrep` match so editors reuse their
/// lists: `env` names the clause and the model, `replacement` is null.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchMatch {
    pub uri: String,
    pub range: Range,
    pub text: String,
    pub env: BTreeMap<String, String>,
    pub replacement: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NotSearched {
    pub model: String,
    pub why: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchReport {
    pub query: String,
    /// Models whose HIR was searched.
    pub searched: usize,
    /// Models the whole query holds in.
    pub models: Vec<String>,
    pub matches: Vec<SearchMatch>,
    pub not_searched: Vec<NotSearched>,
    pub truncated: bool,
}

/// Why a model's HIR cannot be searched, or its sites per clause.
fn model_sites(
    text: &str,
    ctx: &DbtContext,
    query: &query::Query,
) -> Result<Vec<Vec<cogitator_ir::Span>>, String> {
    let doc = cogitator_parser::pipeline::parse_document(text, &ctx.dialect)
        .map_err(|_| "the template does not parse".to_string())?;
    if has_control_block(&doc) {
        return Err("Jinja control blocks: the SQL depends on how they render".into());
    }
    let lowered = build_hir(&template_input(&doc, &ctx.dialect), &SchemaEnv::default());
    let hole = lowered.degraded.iter().any(|d| {
        matches!(
            d.kind,
            DegradationKind::ParseHole | DegradationKind::CapExceeded
        )
    });
    let hir = lowered
        .value
        .as_ref()
        .filter(|_| !hole)
        .ok_or_else(|| "the SQL does not parse into a full query".to_string())?;
    Ok(query.clauses.iter().map(|c| eval::sites(hir, c)).collect())
}

/// Each witnessing clause once, with its sites, in the query's order.
fn witness_sites<'a>(
    clauses: &'a [query::Clause],
    sites: &'a [Vec<cogitator_ir::Span>],
    witnesses: &[usize],
) -> impl Iterator<Item = (&'a query::Clause, &'a Vec<cogitator_ir::Span>)> {
    let mut chosen: Vec<usize> = witnesses.to_vec();
    chosen.sort_unstable();
    chosen.dedup();
    chosen.into_iter().map(|i| (&clauses[i], &sites[i]))
}

/// A match for a model the query holds in without a site of its own (a
/// `not` alone): the top of the file, naming the whole query.
fn whole_model(uri: &str, source: &str, query: &str, model: &str) -> SearchMatch {
    SearchMatch {
        uri: uri.to_string(),
        range: Range::default(),
        text: source.lines().next().unwrap_or_default().to_string(),
        env: BTreeMap::from([
            ("clause".to_string(), query.to_string()),
            ("model".to_string(), model.to_string()),
        ]),
        replacement: None,
    }
}

/// Run `query` over the project's models (or the one named `only`).
pub fn search(
    ctx: &DbtContext,
    text: &str,
    only: Option<&str>,
    read: &dyn Fn(&Path) -> Option<String>,
) -> Result<SearchReport, Guidance> {
    let query = query::parse(text)?;
    let mut names: Vec<&String> = ctx
        .model_detail_map
        .iter()
        .filter(|(name, d)| {
            !d.uri.as_os_str().is_empty() && only.is_none_or(|o| o.eq_ignore_ascii_case(name))
        })
        .map(|(name, _)| name)
        .collect();
    names.sort();
    let mut report = SearchReport {
        query: text.trim().to_string(),
        searched: 0,
        models: Vec::new(),
        matches: Vec::new(),
        not_searched: Vec::new(),
        truncated: false,
    };
    for name in names {
        let path = &ctx.model_detail_map[name].uri;
        let not = |why: String| NotSearched {
            model: name.clone(),
            why,
        };
        let Some(source) = read(path) else {
            report
                .not_searched
                .push(not("the file cannot be read".into()));
            continue;
        };
        let sites = match model_sites(&source, ctx, &query) {
            Ok(sites) => sites,
            Err(why) => {
                report.not_searched.push(not(why));
                continue;
            }
        };
        report.searched += 1;
        let found: Vec<bool> = sites.iter().map(|s| !s.is_empty()).collect();
        if !query.expr.holds(&found) {
            continue;
        }
        report.models.push(name.clone());
        let uri = cogitator_lsp::fs_path_to_uri(path);
        let table = LineTable::new(&source);
        let witnesses = query.expr.witnesses(&found);
        if witnesses.iter().all(|&i| sites[i].is_empty()) {
            // matched only through `not`: no site shows it, the model does
            if report.matches.len() == MAX_MATCHES {
                report.truncated = true;
                return Ok(report);
            }
            report
                .matches
                .push(whole_model(&uri, &source, &query.describe(), name));
            continue;
        }
        for (clause, spans) in witness_sites(&query.clauses, &sites, &witnesses) {
            for span in spans {
                if report.matches.len() == MAX_MATCHES {
                    report.truncated = true;
                    return Ok(report);
                }
                let Some(range) = span_range(&source, &table, *span) else {
                    continue;
                };
                let text = source
                    .get(span.start as usize..span.end as usize)
                    .unwrap_or_default();
                report.matches.push(SearchMatch {
                    uri: uri.clone(),
                    range,
                    text: text.lines().next().unwrap_or_default().to_string(),
                    env: BTreeMap::from([
                        ("clause".to_string(), clause.describe()),
                        ("model".to_string(), name.clone()),
                    ]),
                    replacement: None,
                });
            }
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests;
