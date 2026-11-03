//! textDocument/linkedEditingRange wire types (01 §6.3).

use serde::Serialize;

use super::core::Range;

#[derive(Debug, Serialize)]
pub struct LinkedEditingRanges {
    pub ranges: Vec<Range>,
    #[serde(rename = "wordPattern", skip_serializing_if = "Option::is_none")]
    pub word_pattern: Option<String>,
}
