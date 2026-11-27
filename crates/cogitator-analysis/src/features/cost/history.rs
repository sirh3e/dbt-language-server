//! An optional query-history export (cogitator.yml `cost.query_history`):
//! a JSON array of objects, or JSON lines, as a warehouse's query
//! history page or a `select … from information_schema.jobs` exports it.
//! cogitator never connects to the warehouse for this; it only reads the
//! file. CSV is not read (no CSV parser in the build, and the rules
//! forbid a hand-written one) — export JSON.
//!
//! Recognised keys per row (first alias that is present wins):
//! - the row's node: `unique_id` / `node_id` / `model` (a model name or
//!   a `model.<project>.<name>` id), or `query_id` / `job_id` matched to
//!   the run result's adapter_response;
//! - figures: `bytes_processed` / `total_bytes_processed` / `bytes_scanned`,
//!   `bytes_billed` / `total_bytes_billed`, `slot_ms` / `total_slot_ms`,
//!   `rows_affected` / `rows`.

use std::collections::HashMap;

use cogitator_error::guidance::Guidance;
use serde_json::{Map, Value};

use crate::artifacts::adapter_response::{adapter_stats, text, AdapterStats};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct QueryHistory {
    pub by_query_id: HashMap<String, AdapterStats>,
    /// Keyed by model NAME.
    pub by_model: HashMap<String, AdapterStats>,
    /// Rows that named neither a node nor a query id.
    pub unmatched: usize,
}

impl QueryHistory {
    /// What the history says of a model whose last run had `query_id`.
    pub fn lookup(&self, model: &str, query_id: Option<&str>) -> Option<&AdapterStats> {
        query_id
            .and_then(|id| self.by_query_id.get(id))
            .or_else(|| self.by_model.get(model))
    }
}

pub fn parse_history(text_in: &str, origin: &str) -> Result<QueryHistory, Guidance> {
    let rows = rows(text_in).map_err(|err| {
        Guidance::new("could not read the query-history export.")
            .at(origin.to_string())
            .because(err)
            .fix("point cost.query_history at a JSON array of objects or a JSON-lines file.")
    })?;
    let mut history = QueryHistory::default();
    for row in rows {
        let stats = adapter_stats(&row);
        let node = ["unique_id", "node_id", "model"]
            .iter()
            .find_map(|key| row.get(*key).and_then(text))
            .map(|node| node.rsplit('.').next().unwrap_or_default().to_string());
        let mut matched = false;
        if let Some(id) = stats.query_id.clone() {
            history.by_query_id.insert(id, stats.clone());
            matched = true;
        }
        if let Some(model) = node.filter(|m| !m.is_empty()) {
            history.by_model.insert(model, stats);
            matched = true;
        }
        if !matched {
            history.unmatched += 1;
        }
    }
    Ok(history)
}

fn rows(text_in: &str) -> Result<Vec<Map<String, Value>>, String> {
    let trimmed = text_in.trim_start();
    if trimmed.starts_with('[') {
        let all: Vec<Value> = serde_json::from_str(trimmed).map_err(|e| e.to_string())?;
        return Ok(all.into_iter().filter_map(object).collect());
    }
    serde_json::Deserializer::from_str(trimmed)
        .into_iter::<Value>()
        .map(|value| value.map(object).map_err(|e| e.to_string()))
        .filter_map(Result::transpose)
        .collect()
}

fn object(value: Value) -> Option<Map<String, Value>> {
    match value {
        Value::Object(map) => Some(map),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn array_and_lines_read_alike() {
        let array = r#"[{"model": "orders", "total_bytes_processed": 2048},
                        {"query_id": "q1", "bytes_billed": "10"}, 3, {"other": 1}]"#;
        let lines = "{\"model\": \"orders\", \"total_bytes_processed\": 2048}\n\
                     {\"query_id\": \"q1\", \"bytes_billed\": \"10\"}\n{\"other\": 1}\n";
        for input in [array, lines] {
            let history = parse_history(input, "h.json").unwrap();
            assert_eq!(history.by_model["orders"].bytes_processed, Some(2048));
            assert_eq!(
                history.lookup("x", Some("q1")).unwrap().bytes_billed,
                Some(10)
            );
            assert_eq!(history.unmatched, 1);
        }
    }

    #[test]
    fn unique_ids_resolve_to_model_names() {
        let history =
            parse_history(r#"[{"unique_id": "model.p.customers", "slot_ms": 5}]"#, "h").unwrap();
        assert_eq!(history.lookup("customers", None).unwrap().slot_ms, Some(5));
    }

    #[test]
    fn broken_input_is_guidance() {
        let message = parse_history("[{", "x.json").unwrap_err().to_message();
        assert!(message.contains("x.json"), "{message}");
        assert!(message.contains("JSON"), "{message}");
    }
}
