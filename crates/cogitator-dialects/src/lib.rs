//! SQL dialect + static function documentation (specs/rust-port/04 §3).
//!
//! The data files were extracted from the Go repo's `docs/` map literals
//! (see 00 §porting-order step 5) and load lazily on first use.

use std::collections::HashMap;
use std::sync::LazyLock;

use cogitator_lsp::{completion_kind, CompletionData, CompletionItem};

static SNOWFLAKE: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../data/snowflake.json"))
        .expect("bundled snowflake.json is valid")
});
static BIGQUERY: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../data/bigquery.json"))
        .expect("bundled bigquery.json is valid")
});
static EMPTY: LazyLock<HashMap<String, String>> = LazyLock::new(HashMap::new);

/// The SQL dialect of the active dbt profile — a plain string like Go's
/// `docs.Dialect` ("snowflake", "duckdb", "bigquery", "postgres", "", …).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Dialect(pub String);

impl Dialect {
    pub fn new(s: impl Into<String>) -> Self {
        Dialect(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Function name → markdown docs (04 §3). Only snowflake and bigquery
    /// ship documentation, faithful to Go.
    pub fn function_docs(&self) -> &'static HashMap<String, String> {
        match self.0.as_str() {
            "snowflake" => &SNOWFLAKE,
            "bigquery" => &BIGQUERY,
            _ => &EMPTY,
        }
    }

    /// One completion item per documented function (04 §3).
    pub fn function_completion_items(&self) -> Vec<CompletionItem> {
        self.function_docs()
            .keys()
            .map(|name| CompletionItem {
                label: name.clone(),
                detail: String::new(),
                // resolved lazily via completionItem/resolve — these are the
                // bulkiest documentation strings by far
                documentation: String::new(),
                kind: completion_kind::FUNCTION,
                insert_text: name.clone(),
                sort_text: name.clone(),
                data: Some(CompletionData::Function { name: name.clone() }.into_value()),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docs_load_and_lookup() {
        let d = Dialect::new("snowflake");
        assert!(d.function_docs().len() > 700);
        assert!(d.function_docs().contains_key("abs"));
        assert!(Dialect::new("bigquery").function_docs().len() > 300);
        assert!(Dialect::new("duckdb").function_docs().is_empty());
        assert!(Dialect::new("").function_docs().is_empty());
    }
}
