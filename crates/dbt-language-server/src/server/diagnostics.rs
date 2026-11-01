//! Static diagnostics pipeline (FEATURE_REQUESTS P1/P3): dangling refs,
//! dbt YAML problems and the feature-gated DuckDB 2.0 parse check. Pure over
//! the snapshot + documents; the dispatcher publishes.

use dbt_ls_analysis::{features, yaml_features, DbtContext, Documents};
use dbt_ls_lsp::Diagnostic;

/// Dangling-ref + dbt-YAML + (duckdb) parse diagnostics for one open doc.
pub fn static_diagnostics(ctx: &DbtContext, documents: &Documents, uri: &str) -> Vec<Diagnostic> {
    let Some(doc) = documents.get(uri) else {
        return Vec::new();
    };
    let items = if is_yaml(uri) {
        yaml_features::yaml_diagnostics(uri, &doc.text)
    } else {
        features::dangling_ref_diagnostics(ctx, doc)
    };
    #[cfg(feature = "duckdb-parser")]
    if !is_yaml(uri) && ctx.dialect.as_str() == "duckdb" {
        if let Some(diag) = duckdb_diagnostic(doc) {
            items.push(diag);
        }
    }
    items
}

pub(super) fn is_yaml(uri: &str) -> bool {
    uri.ends_with(".yml") || uri.ends_with(".yaml")
}

/// DuckDB 2.0 parse diagnostic for one document (07 §3), feature-gated.
/// The engine call is blocking but local and milliseconds-scale on
/// model-sized SQL; a dedicated blocking worker is the follow-up if profiling
/// says otherwise.
#[cfg(feature = "duckdb-parser")]
fn duckdb_diagnostic(doc: &dbt_ls_analysis::Document) -> Option<Diagnostic> {
    use std::sync::OnceLock;
    static PARSER: OnceLock<Option<std::sync::Mutex<dbt_ls_duckdb::engine::DuckDbParser>>> =
        OnceLock::new();
    let parser = PARSER
        .get_or_init(|| {
            dbt_ls_duckdb::engine::DuckDbParser::new()
                .map(std::sync::Mutex::new)
                .map_err(|err| tracing::warn!(%err, "duckdb parser unavailable"))
                .ok()
        })
        .as_ref()?;
    let masked = dbt_ls_duckdb::mask_jinja(&doc.parsed)?;
    parser.lock().ok()?.parse_diagnostic(&masked, &doc.text)
}
