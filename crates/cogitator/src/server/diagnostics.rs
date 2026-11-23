//! Static diagnostics pipeline (FEATURE_REQUESTS P1/P3): dangling refs,
//! dbt YAML problems and the feature-gated DuckDB 2.0 parse check. Pure over
//! the snapshot + documents; the dispatcher publishes.

use cogitator_analysis::{features, yaml_features, DbtContext, Document};
use cogitator_lsp::Diagnostic;

/// Dangling-ref + dbt-YAML + (duckdb) parse diagnostics for one open doc
/// (the caller resolves the document — offloaded tasks hold an Arc).
pub fn static_diagnostics(
    ctx: &DbtContext,
    doc: Option<&Document>,
    uri: &str,
    lints: features::lint::LintOptions,
    lineage: Option<&cogitator_analysis::lineage::ModelLineage>,
) -> Vec<Diagnostic> {
    let Some(doc) = doc else {
        return Vec::new();
    };
    let items = if cogitator_analysis::yaml_cogitator::is_cogitator_config(uri) {
        // the server's own config file — never the dbt-properties pass
        cogitator_analysis::yaml_cogitator::cogitator_diagnostics(&doc.text)
    } else if features::selectors_yml::is_selectors_file(uri) {
        // how the selectors are written; the pull path adds what
        // matches nothing (it needs the project's nodes)
        let mut items = yaml_features::yaml_diagnostics(uri, &doc.text);
        items.extend(features::selectors_yml::selectors_diagnostics(&doc.text));
        items
    } else if is_yaml(uri) {
        let mut items = yaml_features::yaml_diagnostics(uri, &doc.text);
        items.extend(features::citizen_diagnostics(ctx, uri));
        items.extend(features::packages_diagnostics(ctx, uri));
        items.extend(features::dangling_doc_diagnostics(ctx, doc));
        items.extend(features::unused_var_diagnostics(ctx, uri, &doc.text));
        items.extend(features::env_var_diagnostics(ctx, doc));
        items
    } else {
        let mut items = features::dangling_ref_diagnostics(ctx, doc);
        items.extend(features::env_var_diagnostics(ctx, doc));
        items.extend(features::private_ref_diagnostics(ctx, doc, uri));
        if let Some(lineage) = lineage {
            items.extend(features::contract_diagnostics(ctx, uri, lineage));
        }
        if lints.any() {
            items.extend(features::lint::lint(ctx, doc, uri, lints, lineage));
        }
        items
    };
    // rebind so the base binding stays immutable in the default build
    #[cfg(feature = "duckdb-parser")]
    let items = {
        let mut items = items;
        if !is_yaml(uri) && ctx.dialect.as_str() == "duckdb" {
            if let Some(diag) = duckdb_diagnostic(doc) {
                items.push(diag);
            }
        }
        items
    };
    items
}

/// The push set (didOpen/didSave, and the breaking-change job's
/// republish): the static diagnostics without lineage, plus what the
/// caches already hold — the LAST loaded artifacts, a plan computed for
/// this exact text, the last breaking-change report. Never touches the
/// filesystem and never plans, so the dispatcher may call it inline.
pub fn push_diagnostics(
    ctx: &std::sync::Arc<DbtContext>,
    doc: Option<&Document>,
    uri: &str,
    lints: features::lint::LintOptions,
    derived: &crate::server::derived_cache::DerivedCache,
    perf: &features::plan::diagnostics_config::PerfSettings,
) -> Vec<Diagnostic> {
    // push stays lineage-free; the pull path adds the lineage-backed checks
    let mut items = static_diagnostics(ctx, doc, uri, lints, None);
    if let Some(root) = ctx.project_root.as_deref() {
        let set = derived.artifacts_cached(root);
        items.extend(features::artifact_diagnostics(ctx, uri, &set));
        if let (true, Some(doc)) = (perf.active(), doc) {
            let key = crate::server::handlers::plan::plan_key(
                ctx,
                derived,
                &set,
                uri,
                perf.catalog_stats,
            );
            items.extend(
                derived
                    .plan_cached(uri, doc.generation, key)
                    .map(|plan| perf_diagnostics(doc, &plan, perf))
                    .unwrap_or_default(),
            );
        }
    }
    items.extend(breaking_diagnostics(derived, uri));
    items
}

/// The last breaking-change report's diagnostics for `uri` (computed on
/// open/save; a cache read only).
pub fn breaking_diagnostics(
    derived: &crate::server::derived_cache::DerivedCache,
    uri: &str,
) -> Vec<Diagnostic> {
    derived
        .breaking_report(uri)
        .map(|report| features::breaking::diagnostics(&report))
        .unwrap_or_default()
}

/// One open model's plan findings as diagnostics, under the live perf
/// settings. The caller supplies the plan: a cache probe on the push
/// path, a computed plan on the pull path.
pub fn perf_diagnostics(
    doc: &Document,
    plan: &crate::server::derived_cache::CachedPlan,
    settings: &features::plan::diagnostics_config::PerfSettings,
) -> Vec<Diagnostic> {
    features::plan::diagnostics::plan_diagnostics(
        &doc.text,
        &plan.plan,
        &plan.suppressions,
        settings,
    )
}

pub(super) fn is_yaml(uri: &str) -> bool {
    uri.ends_with(".yml") || uri.ends_with(".yaml")
}

/// DuckDB 2.0 parse diagnostic for one document (07 §3), feature-gated.
/// The engine call is blocking but local and milliseconds-scale on
/// model-sized SQL; a dedicated blocking worker is the follow-up if profiling
/// says otherwise.
#[cfg(feature = "duckdb-parser")]
fn duckdb_diagnostic(doc: &cogitator_analysis::Document) -> Option<Diagnostic> {
    use std::sync::OnceLock;
    static PARSER: OnceLock<Option<std::sync::Mutex<cogitator_duckdb::engine::DuckDbParser>>> =
        OnceLock::new();
    let parser = PARSER
        .get_or_init(|| {
            cogitator_duckdb::engine::DuckDbParser::new()
                .map(std::sync::Mutex::new)
                .map_err(|err| tracing::warn!(%err, "duckdb parser unavailable"))
                .ok()
        })
        .as_ref()?;
    let masked = cogitator_duckdb::mask_jinja(&doc.parsed)?;
    parser.lock().ok()?.parse_diagnostic(&masked, &doc.text)
}
