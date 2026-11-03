//! sources.json (dbt source freshness output).

use std::path::Path;

use cogitator_error::guidance::Guidance;
use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
pub struct SourcesFreshness {
    #[serde(default)]
    pub results: Vec<FreshnessResult>,
}

#[derive(Debug, Deserialize)]
pub struct FreshnessResult {
    /// "source.<project>.<source>.<table>".
    pub unique_id: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub max_loaded_at: Option<String>,
    #[serde(default)]
    pub snapshotted_at: Option<String>,
}

pub fn parse(text: &str, path: &Path) -> Result<SourcesFreshness, Guidance> {
    serde_json::from_str(text).map_err(|err| {
        Guidance::new("could not read sources.json.")
            .at(path.display().to_string())
            .because(err.to_string())
            .fix("re-run `dbt source freshness`; freshness lenses stay off until then.")
    })
}
