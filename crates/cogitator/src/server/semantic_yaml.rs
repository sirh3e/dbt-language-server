//! `textDocument/semanticTokens/{full,range}` for YAML documents
//! (specs/highlighting §12): the spans `dbt.highlight` answers — the
//! dbt-aware captures, the Jinja and the SQL inside values — through the
//! standard channel, mapped to the legend by
//! `cogitator_analysis::features::encode_captures`.

use cogitator_analysis::features::{encode_captures, tokens_in_lines, CaptureSpan};
use cogitator_lsp::SemanticTokens;
use cogitator_treesitter::HighlightLanguage;

use crate::server::derived_cache::DerivedCache;
use crate::server::snapshot::Snapshot;

/// Is the document one the YAML channel answers for?
pub fn is_yaml(uri: &str) -> bool {
    let path = cogitator_lsp::uri_to_fs_path(uri).unwrap_or_else(|| uri.to_string());
    HighlightLanguage::of_path(&path) == HighlightLanguage::Yaml
}

/// The semantic tokens of a YAML document, all of them or those of lines
/// `start..=end`; empty when the document cannot be highlighted.
pub fn tokens(
    snap: &Snapshot,
    derived: &DerivedCache,
    uri: &str,
    lines: Option<(u32, u32)>,
) -> SemanticTokens {
    let Some(found) = crate::server::highlight::document_highlights(snap, derived, uri) else {
        return SemanticTokens { data: Vec::new() };
    };
    let spans: Vec<CaptureSpan> = found
        .spans
        .iter()
        .filter(|s| s.start.line == s.end.line)
        .map(|s| CaptureSpan {
            line: s.start.line,
            character: s.start.character,
            length: s.end.character.saturating_sub(s.start.character),
            capture: &s.capture,
            layer: s.layer.name(),
        })
        .collect();
    let full = encode_captures(&spans);
    match lines {
        Some((start, end)) => tokens_in_lines(&full, start, end),
        None => full,
    }
}
