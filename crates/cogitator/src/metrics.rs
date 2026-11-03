//! Opt-in OTLP metrics (05 §6.3): free-function recorders that no-op
//! until `observability::init` installs the meter provider (same
//! `--otlp` gate and resource as the trace pipeline). The in-process
//! counters behind dbt.status / dbt.cache keep working unchanged —
//! these recorders observe the same increments at the same sites.

use std::sync::OnceLock;

use opentelemetry::metrics::{Counter, Histogram};
use opentelemetry::KeyValue;

struct Handles {
    requests: Counter<u64>,
    request_duration: Histogram<f64>,
    refreshes: Counter<u64>,
    refresh_duration: Histogram<f64>,
    scan_cache: Counter<u64>,
    derived_cache: Counter<u64>,
    dbt_invocations: Counter<u64>,
}

static HANDLES: OnceLock<Handles> = OnceLock::new();

pub fn init(provider: &opentelemetry_sdk::metrics::SdkMeterProvider) {
    use opentelemetry::metrics::MeterProvider as _;
    let meter = provider.meter("dbt-language-server");
    let _ = HANDLES.set(Handles {
        requests: meter.u64_counter("lsp.requests").build(),
        request_duration: meter
            .f64_histogram("lsp.request.duration")
            .with_unit("ms")
            .build(),
        refreshes: meter.u64_counter("context.refreshes").build(),
        refresh_duration: meter
            .f64_histogram("context.refresh.duration")
            .with_unit("ms")
            .build(),
        scan_cache: meter.u64_counter("scan.cache").build(),
        derived_cache: meter.u64_counter("derived.cache").build(),
        dbt_invocations: meter.u64_counter("dbt.invocations").build(),
    });
}

/// One handled LSP message: count + duration histogram by method
/// (the 05 §6.1 latency-budget measurement).
pub fn record_request(method: &str, ms: f64) {
    if let Some(h) = HANDLES.get() {
        let attrs = [KeyValue::new("method", method.to_string())];
        h.requests.add(1, &attrs);
        h.request_duration.record(ms, &attrs);
    }
}

/// One finished context refresh, with the scan cache's hit/miss DELTAS
/// for this build (the builder's totals are cumulative).
pub fn record_refresh(ms: f64, scan_hits: u64, scan_misses: u64) {
    if let Some(h) = HANDLES.get() {
        h.refreshes.add(1, &[]);
        h.refresh_duration.record(ms, &[]);
        h.scan_cache
            .add(scan_hits, &[KeyValue::new("outcome", "hit")]);
        h.scan_cache
            .add(scan_misses, &[KeyValue::new("outcome", "miss")]);
    }
}

/// One derived-cache lookup (profiles / lineage / results).
pub fn record_derived_cache(surface: &'static str, hit: bool) {
    if let Some(h) = HANDLES.get() {
        h.derived_cache.add(
            1,
            &[
                KeyValue::new("surface", surface),
                KeyValue::new("outcome", if hit { "hit" } else { "miss" }),
            ],
        );
    }
}

/// One dbt subprocess outcome (run/test/compile/parse).
pub fn record_dbt(kind: &'static str, outcome: &'static str) {
    if let Some(h) = HANDLES.get() {
        h.dbt_invocations.add(
            1,
            &[
                KeyValue::new("kind", kind),
                KeyValue::new("outcome", outcome),
            ],
        );
    }
}
