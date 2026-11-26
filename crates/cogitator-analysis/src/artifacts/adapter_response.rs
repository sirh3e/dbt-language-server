//! A run result's `adapter_response`: what the warehouse said about the
//! query. Its shape is the adapter's (BigQuery reports bytes and slots,
//! Snowflake a query id, every adapter `rows_affected`), so it is read
//! out of the raw JSON map leniently: numbers or numeric strings, a few
//! known aliases, everything else ignored.

use serde::Serialize;
use serde_json::{Map, Value};

/// The cost figures of one node's last run; each `None` when the adapter
/// did not report it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterStats {
    pub bytes_processed: Option<u64>,
    pub bytes_billed: Option<u64>,
    pub slot_ms: Option<u64>,
    pub rows_affected: Option<i64>,
    pub query_id: Option<String>,
}

impl AdapterStats {
    /// The bytes the query scanned: processed, else billed.
    pub fn bytes_scanned(&self) -> Option<u64> {
        self.bytes_processed.or(self.bytes_billed)
    }

    pub fn is_empty(&self) -> bool {
        *self == AdapterStats::default()
    }

    /// Missing figures taken from `other` (query history fills gaps,
    /// never overrides what dbt recorded).
    pub fn filled_from(mut self, other: &AdapterStats) -> AdapterStats {
        self.bytes_processed = self.bytes_processed.or(other.bytes_processed);
        self.bytes_billed = self.bytes_billed.or(other.bytes_billed);
        self.slot_ms = self.slot_ms.or(other.slot_ms);
        self.rows_affected = self.rows_affected.or(other.rows_affected);
        if self.query_id.is_none() {
            self.query_id.clone_from(&other.query_id);
        }
        self
    }
}

const BYTES_PROCESSED: &[&str] = &["bytes_processed", "total_bytes_processed", "bytes_scanned"];
const BYTES_BILLED: &[&str] = &["bytes_billed", "total_bytes_billed"];
const SLOT_MS: &[&str] = &["slot_ms", "slot_millis", "total_slot_ms"];
const ROWS: &[&str] = &["rows_affected", "rows"];
const QUERY_ID: &[&str] = &["query_id", "job_id"];

/// Reads the known figures out of an adapter_response map.
pub fn adapter_stats(map: &Map<String, Value>) -> AdapterStats {
    AdapterStats {
        bytes_processed: first(map, BYTES_PROCESSED, unsigned),
        bytes_billed: first(map, BYTES_BILLED, unsigned),
        slot_ms: first(map, SLOT_MS, unsigned),
        rows_affected: first(map, ROWS, signed),
        query_id: first(map, QUERY_ID, text),
    }
}

fn first<T>(map: &Map<String, Value>, keys: &[&str], read: fn(&Value) -> Option<T>) -> Option<T> {
    keys.iter().find_map(|key| map.get(*key).and_then(read))
}

pub(crate) fn unsigned(value: &Value) -> Option<u64> {
    match value {
        Value::Number(n) => n
            .as_u64()
            .or_else(|| n.as_f64().filter(|f| *f >= 0.0).map(|f| f as u64)),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn signed(value: &Value) -> Option<i64> {
    match value {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

pub(crate) fn text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) if !s.trim().is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// `3.2 GB` — binary units, one decimal from KB on.
pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(json: &str) -> Map<String, Value> {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn bigquery_shape() {
        let stats = adapter_stats(&map(
            r#"{"_message": "CREATE TABLE (9.0 rows, 3.2 GiB processed)", "code": "CREATE TABLE",
                "rows_affected": 9, "bytes_processed": 3435973837, "bytes_billed": "3435973837",
                "slot_ms": 1234.0, "job_id": "abc-1", "location": "EU"}"#,
        ));
        assert_eq!(stats.bytes_processed, Some(3_435_973_837));
        assert_eq!(stats.bytes_billed, Some(3_435_973_837));
        assert_eq!(stats.slot_ms, Some(1234));
        assert_eq!(stats.rows_affected, Some(9));
        assert_eq!(stats.query_id.as_deref(), Some("abc-1"));
        assert_eq!(human_bytes(stats.bytes_scanned().unwrap()), "3.2 GB");
    }

    #[test]
    fn odd_shapes_degrade_per_field() {
        let stats = adapter_stats(&map(
            r#"{"rows_affected": "many", "bytes_processed": [1], "query_id": "", "rows": -1}"#,
        ));
        assert_eq!(stats.bytes_processed, None);
        assert_eq!(stats.rows_affected, Some(-1), "the alias answers");
        assert_eq!(stats.query_id, None);
        assert!(adapter_stats(&Map::new()).is_empty());
    }

    #[test]
    fn history_fills_gaps_only() {
        let dbt = AdapterStats {
            bytes_processed: Some(1),
            ..Default::default()
        };
        let history = AdapterStats {
            bytes_processed: Some(9),
            slot_ms: Some(5),
            ..Default::default()
        };
        let merged = dbt.filled_from(&history);
        assert_eq!(merged.bytes_processed, Some(1));
        assert_eq!(merged.slot_ms, Some(5));
    }

    #[test]
    fn human_bytes_units() {
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(2048), "2.0 KB");
        assert_eq!(human_bytes(5 * 1024 * 1024 * 1024 * 1024), "5.0 TB");
    }
}
