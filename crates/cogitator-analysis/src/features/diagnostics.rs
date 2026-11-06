//! Dangling-reference diagnostics (FEATURE_REQUESTS P1, graph-adjacent):
//! unknown `ref()` models, unknown `source()` names and unknown `var()`s,
//! each with a did-you-mean suggestion when a near miss exists.

use cogitator_error::guidance::{closest_matches, Guidance};
use cogitator_lsp::{diagnostic_severity, Diagnostic};
use cogitator_parser::TokenKind;

use super::{token_range, DbtContext, Document};

/// Dangling ref/source/var warnings. Each kind is suppressed while its
/// collection is still empty (the first context refresh pending would
/// flag everything).
pub fn dangling_ref_diagnostics(ctx: &DbtContext, doc: &Document) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for t in &doc.parsed.tokens {
        let literal = doc.parsed.literal(t);
        let diagnostic = match t.kind {
            TokenKind::Ref if !ctx.model_detail_map.is_empty() => (!ctx
                .model_detail_map
                .contains_key(literal))
            .then(|| {
                entry(
                    t,
                    "dangling-ref",
                    Guidance::new(format!("model '{literal}' not found in the project")).suggest(
                        closest_matches(literal, ctx.model_detail_map.keys().map(String::as_str)),
                    ),
                )
            }),
            TokenKind::SourceName if !ctx.source_detail_map.is_empty() => {
                (!ctx.source_detail_map.contains_key(literal)).then(|| {
                    entry(
                        t,
                        "dangling-source",
                        Guidance::new(format!("source '{literal}' is not declared in any YAML"))
                            .suggest(closest_matches(
                                literal,
                                ctx.source_detail_map.keys().map(String::as_str),
                            )),
                    )
                })
            }
            // var('name', default) is legitimate without a project var —
            // only defaultless lookups can dangle
            TokenKind::Var if !ctx.variable_detail_map.is_empty() && !has_default(doc, t) => {
                (!ctx.variable_detail_map.contains_key(literal)).then(|| {
                    entry(
                        t,
                        "unknown-var",
                        Guidance::new(format!("var '{literal}' is not defined in dbt_project.yml"))
                            .suggest(closest_matches(
                                literal,
                                ctx.variable_detail_map.keys().map(String::as_str),
                            )),
                    )
                })
            }
            _ => None,
        };
        out.extend(diagnostic.flatten());
    }
    out.extend(dangling_doc_diagnostics(ctx, doc));
    out
}

/// `doc('name')` with no docs block anywhere renders verbatim in dbt
/// docs — flag it with the closest block names. Separate from the ref
/// pass because doc() mostly lives in properties YAML, which skips
/// the SQL dangling checks.
pub fn dangling_doc_diagnostics(ctx: &DbtContext, doc: &Document) -> Vec<Diagnostic> {
    if ctx.doc_detail_map.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for t in &doc.parsed.tokens {
        if t.kind != TokenKind::Doc {
            continue;
        }
        let literal = doc.parsed.literal(t);
        if ctx.doc_detail_map.contains_key(literal) {
            continue;
        }
        out.extend(entry(
            t,
            "dangling-doc",
            Guidance::new(format!("docs block '{literal}' not found in any docs path")).suggest(
                closest_matches(literal, ctx.doc_detail_map.keys().map(String::as_str)),
            ),
        ));
    }
    out
}

/// Does this `var()` call carry a second (default) argument?
fn has_default(doc: &Document, token: &cogitator_parser::Token) -> bool {
    let rest = &doc.parsed.text[token.end as usize..];
    let close = rest.find(')').unwrap_or(0);
    rest[..close].contains(',')
}

fn entry(token: &cogitator_parser::Token, code: &str, guidance: Guidance) -> Option<Diagnostic> {
    let range = token_range(token);
    let mut diagnostic = Diagnostic::plain(
        range,
        guidance.to_diagnostic(),
        diagnostic_severity::WARNING,
        code,
        "cogitator",
    );
    // the suggestions ride as typed data so quickfixes never re-derive
    // them from the message text (01 §6.1)
    if !guidance.suggestions.is_empty() {
        diagnostic.data = Some(cogitator_lsp::DiagnosticData {
            fix: cogitator_lsp::FixKind::ReplaceToken,
            token_range: range,
            suggestions: guidance.suggestions,
        });
    }
    Some(diagnostic)
}

/// Dangling exposure/metric/semantic-model targets in THIS properties
/// file: a depends entry pointing at no model/source warns at the
/// citizen's own entry, with did-you-mean over the model names.
pub fn citizen_diagnostics(ctx: &DbtContext, uri: &str) -> Vec<Diagnostic> {
    use crate::context::exposures::RefTarget;

    let path = cogitator_lsp::uri_to_fs_path(uri).unwrap_or_else(|| uri.to_string());
    let path = std::path::Path::new(&path);
    let mut out = Vec::new();
    let citizens = ctx
        .exposure_detail_map
        .values()
        .map(|c| ("exposure", c))
        .chain(ctx.metric_detail_map.values().map(|c| ("metric", c)))
        .chain(
            ctx.semantic_model_detail_map
                .values()
                .map(|c| ("semantic model", c)),
        );
    for (what, citizen) in citizens {
        if citizen.uri.as_path() != path {
            continue;
        }
        for target in &citizen.depends {
            let guidance = match target {
                RefTarget::Model(model) if !ctx.model_detail_map.contains_key(model) => {
                    Guidance::new(format!(
                        "{what} '{}' depends on unknown model '{model}'",
                        citizen.name
                    ))
                    .suggest(closest_matches(
                        model,
                        ctx.model_detail_map.keys().map(String::as_str),
                    ))
                }
                RefTarget::Source(source, _) if !ctx.source_detail_map.contains_key(source) => {
                    Guidance::new(format!(
                        "{what} '{}' depends on unknown source '{source}'",
                        citizen.name
                    ))
                    .suggest(closest_matches(
                        source,
                        ctx.source_detail_map.keys().map(String::as_str),
                    ))
                }
                _ => continue,
            };
            out.push(Diagnostic::plain(
                citizen.range,
                guidance.to_diagnostic(),
                diagnostic_severity::WARNING,
                "dangling-exposure-ref",
                "cogitator",
            ));
        }
    }
    out
}

/// packages.yml / dependencies.yml: declared packages whose install
/// directory is missing — everything that depends on them is degraded
/// until `dbt deps` runs.
pub fn packages_diagnostics(ctx: &DbtContext, uri: &str) -> Vec<Diagnostic> {
    let is_packages_file = std::path::Path::new(uri)
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n == "packages.yml" || n == "dependencies.yml");
    if !is_packages_file {
        return Vec::new();
    }
    // installed = every project name the scan saw besides our own
    let own = ctx.project.name.as_str();
    let installed: std::collections::HashSet<&str> = ctx
        .model_detail_map
        .values()
        .map(|m| m.project_name.as_str())
        .chain(ctx.macro_detail_map.keys().map(String::as_str))
        .filter(|name| *name != own && !name.is_empty())
        .collect();
    ctx.declared_packages
        .iter()
        .filter(|declared| !installed.contains(declared.as_str()))
        .map(|declared| {
            Diagnostic::plain(
                cogitator_lsp::Range::default(),
                Guidance::new(format!(
                    "package '{declared}' is declared but not installed — run `dbt deps`"
                ))
                .to_diagnostic(),
                diagnostic_severity::WARNING,
                "package-not-installed",
                "cogitator",
            )
        })
        .collect()
}
