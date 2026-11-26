//! The column sets a model is declared unique on, from its YAML entry
//! and the manifest, for `merge::ModelFacts::keys`.

/// Column sets declared unique: model-level constraints and combination
/// tests, and the manifest's `unique_key` of an incremental model (the
/// template's own is read at inference).
pub(super) fn declared_keys(
    details: Option<&crate::ModelDetails>,
    manifest: Option<&crate::artifacts::manifest::ManifestIndex>,
    model: &str,
) -> Vec<Vec<String>> {
    let mut keys: Vec<Vec<String>> = details.map(|d| d.unique_keys.clone()).unwrap_or_default();
    if let Some(config) = manifest
        .and_then(|m| m.models.get(model))
        .map(|n| &n.config)
    {
        let incremental = config
            .materialized
            .as_deref()
            .is_some_and(|m| m.eq_ignore_ascii_case("incremental"));
        if incremental && !config.unique_key.is_empty() {
            keys.push(
                config
                    .unique_key
                    .iter()
                    .map(|k| k.to_ascii_lowercase())
                    .collect(),
            );
        }
    }
    keys
}
