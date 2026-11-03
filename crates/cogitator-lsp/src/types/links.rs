//! textDocument/documentLink wire types (01 §6.3). No resolve support:
//! every link ships its target up front.

use serde::Serialize;

use super::core::Range;

#[derive(Debug, Serialize)]
pub struct DocumentLink {
    pub range: Range,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentLinkOptions {
    pub resolve_provider: bool,
}
