//! Outgoing result/notification types (01 §2.3 + post-parity).

use serde::Serialize;

use super::core::{Location, Range};

// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct ResultResponse<T> {
    pub jsonrpc: &'static str,
    pub id: Option<i64>,
    pub result: T,
}

impl<T> ResultResponse<T> {
    pub fn new(id: i64, result: T) -> Self {
        ResultResponse {
            jsonrpc: "2.0",
            id: Some(id),
            result,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct HoverResult {
    pub contents: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CompletionItem {
    pub label: String,
    pub detail: String,
    pub documentation: String,
    pub kind: i32,
    #[serde(rename = "insertText")]
    pub insert_text: String,
    #[serde(rename = "sortText")]
    pub sort_text: String,
    /// Opaque payload for `completionItem/resolve` (additive; absent in the
    /// Go shape, omitted when unused).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// LSP CompletionItemKind values used by the server (01 §2.3).
pub mod completion_kind {
    pub const FUNCTION: i32 = 3;
    pub const VARIABLE: i32 = 6;
    pub const SNIPPET: i32 = 15;
    pub const REFERENCE: i32 = 18;
}

/// LSP DiagnosticSeverity values.
pub mod diagnostic_severity {
    pub const ERROR: i32 = 1;
    pub const WARNING: i32 = 2;
    pub const INFO: i32 = 3;
    pub const HINT: i32 = 4;
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub range: Range,
    pub message: String,
    pub severity: i32,
    pub code: String,
    pub source: String,
}

#[derive(Debug, Serialize)]
pub struct Notification<T> {
    pub jsonrpc: &'static str,
    pub method: &'static str,
    pub params: T,
}

#[derive(Debug, Serialize)]
pub struct PublishDiagnosticsParams {
    pub uri: String,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn publish_diagnostics(
    uri: String,
    diagnostics: Vec<Diagnostic>,
) -> Notification<PublishDiagnosticsParams> {
    Notification {
        jsonrpc: "2.0",
        method: "textDocument/publishDiagnostics",
        params: PublishDiagnosticsParams { uri, diagnostics },
    }
}

pub mod symbol_kind {
    pub const NAMESPACE: i32 = 3;
    pub const CLASS: i32 = 5;
    pub const INTERFACE: i32 = 11;
    pub const FUNCTION: i32 = 12;
    pub const VARIABLE: i32 = 13;
}

#[derive(Debug, Clone, Serialize)]
pub struct SymbolInformation {
    pub name: String,
    pub kind: i32,
    pub location: Location,
}

#[derive(Debug, Clone, Serialize)]
pub struct DocumentHighlight {
    pub range: Range,
    pub kind: i32, // 1 = Text
}

#[derive(Debug, Clone, Serialize)]
pub struct FoldingRange {
    #[serde(rename = "startLine")]
    pub start_line: u32,
    #[serde(rename = "endLine")]
    pub end_line: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct SelectionRange {
    pub range: Range,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<Box<SelectionRange>>,
}

#[derive(Debug, Serialize)]
pub struct SemanticTokens {
    pub data: Vec<u32>,
}

#[derive(Debug, Serialize)]
pub struct SignatureHelp {
    pub signatures: Vec<SignatureInformation>,
    #[serde(rename = "activeSignature")]
    pub active_signature: u32,
    #[serde(rename = "activeParameter")]
    pub active_parameter: u32,
}

#[derive(Debug, Serialize)]
pub struct SignatureInformation {
    pub label: String,
    pub parameters: Vec<ParameterInformation>,
}

#[derive(Debug, Serialize)]
pub struct ParameterInformation {
    pub label: String,
}

/// Pull-diagnostics full report (textDocument/diagnostic).
#[derive(Debug, Serialize)]
pub struct FullDocumentDiagnosticReport {
    pub kind: &'static str, // "full"
    pub items: Vec<Diagnostic>,
}
