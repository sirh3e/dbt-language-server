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
    /// Plain-text tooltip (a var's whole value and where it is defined).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<String>,
}

/// One part of a parted inlay label (LSP 3.17 `InlayHintLabelPart`):
/// its own tooltip, and a command a click runs.
#[derive(Debug, Clone, Serialize)]
pub struct InlayHintLabelPart {
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<super::results::Command>,
}

/// An inlay hint whose label is a list of parts (the plan findings').
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartedInlayHint {
    pub position: Position,
    pub label: Vec<InlayHintLabelPart>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<u8>,
    pub padding_left: bool,
    pub padding_right: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<String>,
}

/// Either label shape; serialises as the hint itself.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum AnyInlayHint {
    Plain(InlayHint),
    Parted(PartedInlayHint),
}
