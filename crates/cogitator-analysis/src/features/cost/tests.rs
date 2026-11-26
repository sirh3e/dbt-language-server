use std::path::Path;
use std::sync::Arc;

use super::*;
use crate::artifacts::run_results;

fn set(json: &str) -> ArtifactSet {
    ArtifactSet {
        run_results: Some(Arc::new(run_results::parse(json, Path::new("rr")).unwrap())),
        ..Default::default()
    }
}

const RUN: &str = r#"{"metadata": {"generated_at": "2026-11-20T08:00:00Z"}, "results": [
    {"unique_id": "model.p.big", "status": "success", "execution_time": 12.4,
     "adapter_response": {"bytes_processed": 3435973837, "slot_ms": 900, "query_id": "q-big"}},
    {"unique_id": "model.p.mid", "status": "success", "execution_time": 2.0},
    {"unique_id": "model.p.small", "status": "success", "execution_time": 0.5},
    {"unique_id": "model.p.tiny", "status": "success", "execution_time": 0.1},
    {"unique_id": "test.p.t.x", "status": "pass", "execution_time": 99.0}
]}"#;

#[test]
fn slowest_percent_rounds_up() {
    let all = [12.4, 2.0, 0.5, 0.1];
    assert_eq!(slowest_percent(12.4, &all), 25);
    assert_eq!(slowest_percent(0.1, &all), 100);
    assert_eq!(slowest_percent(1.0, &[1.0; 20]), 5);
}

#[test]
fn lens_parts_name_bytes_and_share() {
    let set = set(RUN);
    let rr = set.run_results.as_deref().unwrap();
    let statuses = run_status_by_model(rr);
    assert_eq!(
        lens_parts(rr, "big", &statuses["big"]),
        ["3.2 GB scanned", "slowest 25%"]
    );
    assert!(lens_parts(rr, "tiny", &statuses["tiny"]).is_empty());
}

#[test]
fn report_ranks_models_and_fills_from_history() {
    let set = set(RUN);
    let history = parse_history(
        r#"[{"query_id": "q-big", "bytes_billed": 10, "bytes_processed": 1},
            {"model": "mid", "total_bytes_processed": 2048}]"#,
        "h",
    )
    .unwrap();
    let report = cost_report(&DbtContext::default(), &set, None, Some(&history));
    let names: Vec<&str> = report.models.iter().map(|m| m.model.as_str()).collect();
    assert_eq!(
        names,
        ["big", "mid", "small", "tiny"],
        "tests are not models"
    );
    assert_eq!(
        report.models[0].adapter.bytes_processed,
        Some(3_435_973_837)
    );
    assert_eq!(
        report.models[0].adapter.bytes_billed,
        Some(10),
        "gap filled"
    );
    assert_eq!(report.models[1].adapter.bytes_processed, Some(2048));
    assert_eq!(report.history_rows, 2);
    assert_eq!(report.total_seconds, 15.0);
    assert!(report.text.contains("3.2 GB scanned"), "{}", report.text);
}

#[test]
fn no_run_results_says_so() {
    let report = cost_report(&DbtContext::default(), &ArtifactSet::default(), None, None);
    assert!(report.models.is_empty());
    assert!(report.text.contains("run dbt"), "{}", report.text);
}
