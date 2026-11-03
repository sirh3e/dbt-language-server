//! sources.json (dbt source freshness output), through the generated
//! sources-v3 types — RuntimeError and Output rows both land in the
//! merged open shape the preprocess artifact mode emits (08 §4).

use std::path::Path;

use cogitator_error::guidance::Guidance;

#[derive(Debug, Default)]
pub struct SourcesFreshness {
    pub results: Vec<FreshnessResult>,
}

#[derive(Debug)]
pub struct FreshnessResult {
    /// "source.<project>.<source>.<table>".
    pub unique_id: String,
    pub status: String,
    pub max_loaded_at: Option<String>,
    pub snapshotted_at: Option<String>,
}

pub fn parse(text: &str, path: &Path) -> Result<SourcesFreshness, Guidance> {
    use cogitator_schemas::artifacts::sources as gen;
    let raw: gen::FreshnessExecutionResultArtifact = serde_json::from_str(text).map_err(|err| {
        Guidance::new("could not read sources.json.")
            .at(path.display().to_string())
            .because(err.to_string())
            .fix("re-run `dbt source freshness`; freshness lenses stay off until then.")
    })?;
    super::trace_schema_version(
        path,
        raw.metadata
            .as_ref()
            .and_then(|m| m.dbt_schema_version.as_deref()),
        cogitator_schemas::artifacts::SOURCES_SCHEMA_V3,
    );
    Ok(SourcesFreshness {
        results: raw
            .results
            .into_iter()
            .filter_map(|result| {
                let Some(unique_id) = result.unique_id else {
                    tracing::debug!("freshness result without unique_id skipped");
                    return None;
                };
                Some(FreshnessResult {
                    unique_id,
                    status: result.status.unwrap_or_default(),
                    max_loaded_at: result.max_loaded_at,
                    snapshotted_at: result.snapshotted_at,
                })
            })
            .collect(),
    })
}
