//! run_results.json: per-node status, timing and adapter message.

use std::path::Path;

use cogitator_error::guidance::Guidance;
use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
pub struct RunResults {
    #[serde(default)]
    pub metadata: ArtifactMeta,
    #[serde(default)]
    pub results: Vec<RunResult>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ArtifactMeta {
    #[serde(default)]
    pub generated_at: Option<String>,
    #[serde(default)]
    pub dbt_version: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RunResult {
    /// The only hard requirement: "model.<project>.<name>",
    /// "test.<project>.<name>.<hash>", "seed.…", "source.…".
    pub unique_id: String,
    /// "success" | "error" | "fail" | "skipped" | "pass" | "warn" — or
    /// whatever a future dbt says; consumers match loosely.
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub execution_time: f64,
    #[serde(default)]
    pub message: Option<String>,
}

pub fn parse(text: &str, path: &Path) -> Result<RunResults, Guidance> {
    serde_json::from_str(text).map_err(|err| {
        Guidance::new("could not read run_results.json.")
            .at(path.display().to_string())
            .because(err.to_string())
            .fix("re-run dbt to regenerate target/; run-status features stay off until then.")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_and_missing_fields_are_tolerated() {
        let rr = parse(
            r#"{"metadata": {"generated_at": "2026-11-03T10:00:00Z", "env": {"x": 1}},
                "results": [
                  {"unique_id": "model.jaffle_shop.customers", "status": "success",
                   "execution_time": 3.2, "adapter_response": {"rows": 9}},
                  {"unique_id": "test.jaffle_shop.not_null_customers_customer_id.abc",
                   "status": "fail", "message": "Got 3 results"}
                ],
                "elapsed_time": 5.0, "args": {"which": "build"}}"#,
            Path::new("run_results.json"),
        )
        .unwrap();
        assert_eq!(rr.results.len(), 2);
        assert_eq!(rr.results[0].status, "success");
        assert_eq!(rr.results[1].execution_time, 0.0);
        assert_eq!(rr.metadata.dbt_version, None);
    }

    #[test]
    fn broken_json_is_guidance_not_a_panic() {
        let err = parse("{not json", Path::new("target/run_results.json")).unwrap_err();
        let message = err.to_message();
        assert!(message.contains("run_results.json"), "{message}");
        assert!(message.contains("re-run dbt"), "{message}");
    }
}
