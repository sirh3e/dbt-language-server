//! textDocument/inlayHint wire types (01 §6.3).

use serde::{Deserialize, Serialize};

use super::core::{Position, Range};
use super::params::TextDocumentIdentifier;

#[derive(Debug, Deserialize)]
pub struct InlayHintParams<'a> {
    #[serde(borrow, rename = "textDocument")]
    pub text_document: TextDocumentIdentifier<'a>,
    /// An all-zero range means "whole document" here (clients always
    /// send one; a default only appears in tests).
    #[serde(default)]
    pub range: Range,
}

pub mod inlay_hint_kind {
    pub const TYPE: u8 = 1;
    pub const PARAMETER: u8 = 2;
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InlayHint {
    pub position: Position,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<u8>,
    pub padding_left: bool,
    pub padding_right: bool,
}
