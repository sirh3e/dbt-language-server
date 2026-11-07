//! Outgoing result/notification types (01 §2.3 + post-parity).

use serde::{Deserialize, Serialize};

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

/// The typed shape inside `CompletionItem.data` (the field itself stays an
/// opaque `Value`, as the protocol round-trips it through the client).
/// `kind` is the tag; variant payloads name what `completionItem/resolve`
/// needs to find the documentation again.
#[derive(Debug, Clone, serde::Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum CompletionData {
    Model {
        name: String,
    },
    Source {
        name: String,
        table: String,
    },
    Variable {
        name: String,
    },
    Doc {
        name: String,
    },
    Macro {
        name: String,
        /// Tolerates clients echoing data without the package (the
        /// resolver falls back to the project package).
        #[serde(skip_serializing_if = "Option::is_none")]
        package: Option<String>,
    },
    Function {
        name: String,
    },
}

impl CompletionData {
    /// Into the opaque `CompletionItem.data` payload.
    pub fn into_value(self) -> serde_json::Value {
        serde_json::to_value(self).expect("completion data serializes")
    }
}

/// LSP CompletionItemKind values used by the server (01 §2.3).
pub mod completion_kind {
    pub const TEXT: i32 = 1;
    pub const FUNCTION: i32 = 3;
    pub const FIELD: i32 = 5;
    pub const VARIABLE: i32 = 6;
    pub const SNIPPET: i32 = 15;
    pub const KEYWORD: i32 = 14;
    pub const REFERENCE: i32 = 18;
}

/// LSP DiagnosticSeverity values.
pub mod diagnostic_severity {
    pub const ERROR: i32 = 1;
    pub const WARNING: i32 = 2;
    pub const INFO: i32 = 3;
    pub const HINT: i32 = 4;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub range: Range,
    pub message: String,
    #[serde(default)]
    pub severity: i32,
    #[serde(default, deserialize_with = "super::diagnostics::lenient_code")]
    pub code: String,
    #[serde(default)]
    pub source: String,
    /// LSP DiagnosticTags (1 = Unnecessary, 2 = Deprecated).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<u8>>,
    #[serde(
        default,
        rename = "relatedInformation",
        skip_serializing_if = "Option::is_none"
    )]
    pub related_information: Option<Vec<super::diagnostics::DiagnosticRelatedInformation>>,
    /// Typed quickfix payload (01 §6.1); foreign shapes decode to None.
    #[serde(
        default,
        deserialize_with = "super::diagnostics::lenient_data",
        skip_serializing_if = "Option::is_none"
    )]
    pub data: Option<super::diagnostics::DiagnosticData>,
}

impl Diagnostic {
    /// A diagnostic with none of the optional extras — the default
    /// constructor everywhere a plain message is enough.
    pub fn plain(
        range: Range,
        message: String,
        severity: i32,
        code: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Diagnostic {
            range,
            message,
            severity,
            code: code.into(),
            source: source.into(),
            tags: None,
            related_information: None,
            data: None,
        }
    }
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
    /// Used for selectors.yml entries (selector search).
    pub const KEY: i32 = 20;
    /// Used for exposures/metrics/semantic models (exposure search).
    pub const EVENT: i32 = 24;
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

// ---------------------------------------------------------------------------
// CodeLens + window message types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct Command {
    pub title: String,
    pub command: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub arguments: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CodeLens {
    pub range: Range,
    pub command: Command,
}

pub mod message_type {
    pub const ERROR: i32 = 1;
    pub const WARNING: i32 = 2;
    pub const INFO: i32 = 3;
    pub const LOG: i32 = 4;
}

/// Server→client request envelope (window/showDocument, …). The server
/// fire-and-forgets these; client responses are ignored by the dispatcher's
/// response guard.
#[derive(Debug, Serialize)]
pub struct ServerRequest<T> {
    pub jsonrpc: &'static str,
    pub id: i64,
    pub method: &'static str,
    pub params: T,
}

#[derive(Debug, Serialize)]
pub struct ShowMessageParams {
    #[serde(rename = "type")]
    pub message_type: i32,
    pub message: String,
}

/// window/showMessage notification with the reason/solution text inline.
pub fn show_message(
    message_type: i32,
    message: impl Into<String>,
) -> Notification<ShowMessageParams> {
    Notification {
        jsonrpc: "2.0",
        method: "window/showMessage",
        params: ShowMessageParams {
            message_type,
            message: message.into(),
        },
    }
}

/// window/logMessage: the editor's LOG VIEW, not an alert — same
/// params shape as showMessage (01 §5).
pub fn log_message(message_type: i32, message: String) -> Notification<ShowMessageParams> {
    Notification {
        jsonrpc: "2.0",
        method: "window/logMessage",
        params: ShowMessageParams {
            message_type,
            message,
        },
    }
}

#[derive(Debug, Serialize)]
pub struct ShowDocumentParams {
    pub uri: String,
    #[serde(rename = "takeFocus")]
    pub take_focus: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection: Option<Range>,
}
