//! Dangling-ref diagnostics (FEATURE_REQUESTS P1, graph-adjacent).

use dbt_ls_lsp::{diagnostic_severity, Diagnostic};
use dbt_ls_parser::TokenKind;

use super::{token_range, DbtContext, Document};

/// Dangling-ref warnings (P1). Suppressed while the first context refresh is
/// still pending (an empty model map would flag everything).
pub fn dangling_ref_diagnostics(ctx: &DbtContext, doc: &Document) -> Vec<Diagnostic> {
    if ctx.model_detail_map.is_empty() {
        return Vec::new();
    }
    doc.parsed
        .tokens
        .iter()
        .filter(|t| t.kind == TokenKind::Ref)
        .filter(|t| !ctx.model_detail_map.contains_key(doc.parsed.literal(t)))
        .map(|t| Diagnostic {
            range: token_range(t),
            message: format!("model '{}' not found in the project", doc.parsed.literal(t)),
            severity: diagnostic_severity::WARNING,
            code: "dangling-ref".to_string(),
            source: "dbt-language-server".to_string(),
        })
        .collect()
}
