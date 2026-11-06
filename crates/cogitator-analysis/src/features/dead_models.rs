//! Dead-model detection (03 §10): a model nothing transitively
//! depends on — no model, exposure, metric or semantic model — and
//! that no selectors.yml definition names verbatim. Conservative on
//! purpose: seeds and package models are never flagged.

use serde::Serialize;

use super::DbtContext;

#[derive(Debug, Serialize)]
pub struct DeadModel {
    pub name: String,
    pub uri: Option<String>,
    pub reasons: Vec<String>,
}

/// The dbt.deadModels report, name-sorted.
pub fn dead_models(ctx: &DbtContext) -> Vec<DeadModel> {
    let mut out: Vec<DeadModel> = ctx
        .model_detail_map
        .iter()
        .filter(|(name, details)| is_dead(ctx, name, details))
        .map(|(name, details)| DeadModel {
            name: name.clone(),
            uri: (!details.uri.as_os_str().is_empty())
                .then(|| cogitator_lsp::fs_path_to_uri(&details.uri)),
            reasons: vec![
                "nothing downstream depends on it".to_string(),
                "no exposure, metric or semantic model references it".to_string(),
                "no selectors.yml definition names it".to_string(),
            ],
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Is this model dead? (Also the lint.dead_models per-file check.)
pub fn is_dead(ctx: &DbtContext, name: &str, details: &crate::context::ModelDetails) -> bool {
    // seeds and snapshots stay out — their "unused" is normal (a seed
    // backs ad-hoc queries; a snapshot is its own end product) — and so
    // do package models
    if details.kind != crate::context::ModelKind::Model {
        return false;
    }
    if details.project_name != ctx.project.name {
        return false;
    }
    // any transitive dependent — model or citizen — means alive
    if !ctx.graph.transitive_downstream(name).is_empty() {
        return false;
    }
    // a verbatim mention in any selector definition means alive
    let named = ctx.selector_detail_map.values().any(|selector| {
        selector
            .definition_text
            .match_indices(name)
            .any(|(at, hit)| {
                let bytes = selector.definition_text.as_bytes();
                let before_ok = at == 0 || !is_word(bytes[at - 1]);
                let end = at + hit.len();
                let after_ok = end >= bytes.len() || !is_word(bytes[end]);
                before_ok && after_ok
            })
    });
    !named
}

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}
