//! Protocol structs (specs/rust-port/01 §2–§3).
//!
//! Inbound params borrow from the frame buffer where serde allows
//! (`Cow` + `#[serde(borrow)]`, 08 §1.2); outbound types own their data.

use std::borrow::Cow;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Common structures (01 §2.2)
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    #[serde(default)]
    pub line: u32,
    #[serde(default)]
    pub character: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Range {
    #[serde(default)]
    pub start: Position,
    #[serde(default)]
    pub end: Position,
}

impl Range {
    pub fn zero_width(pos: Position) -> Self {
        Range {
            start: pos,
            end: pos,
        }
    }
    pub fn is_zero(&self) -> bool {
        *self == Range::default()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Location {
    pub uri: String,
    pub range: Range,
}

// ---------------------------------------------------------------------------
// Base messages (01 §2.1)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct BaseMessage<'a> {
    #[serde(borrow, default)]
    pub method: Option<Cow<'a, str>>,
}

#[derive(Debug, Deserialize)]
pub struct RequestId {
    #[serde(default)]
    pub id: i64,
}

#[derive(Debug, Serialize)]
pub struct Response {
    pub jsonrpc: &'static str,
    pub id: Option<i64>,
}

impl Response {
    pub fn new(id: i64) -> Self {
        Response {
            jsonrpc: "2.0",
            id: Some(id),
        }
    }
}

// ---------------------------------------------------------------------------
// Incoming params (01 §2.3)
// ---------------------------------------------------------------------------

/// Request/notification envelope. The lifetime-carrying param structs below
/// borrow from the deserializer input through their own `#[serde(borrow)]`
/// fields (08 §1.2).
#[derive(Debug, Deserialize)]
pub struct Envelope<P> {
    #[serde(default)]
    pub id: i64,
    #[serde(default = "Option::default")]
    pub params: Option<P>,
}

#[derive(Debug, Default, Deserialize)]
pub struct InitializeParams<'a> {
    #[serde(borrow, rename = "clientInfo", default)]
    pub client_info: Option<ClientInfo<'a>>,
    #[serde(borrow, rename = "rootPath", default)]
    pub root_path: Option<Cow<'a, str>>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ClientInfo<'a> {
    #[serde(borrow, default)]
    pub name: Cow<'a, str>,
    #[serde(borrow, default)]
    pub version: Cow<'a, str>,
}

#[derive(Debug, Deserialize)]
pub struct TextDocumentIdentifier<'a> {
    #[serde(borrow)]
    pub uri: Cow<'a, str>,
}

#[derive(Debug, Deserialize)]
pub struct TextDocumentItem<'a> {
    #[serde(borrow)]
    pub uri: Cow<'a, str>,
    #[serde(borrow, rename = "languageId", default)]
    pub language_id: Option<Cow<'a, str>>,
    #[serde(default)]
    pub version: i64,
    #[serde(borrow)]
    pub text: Cow<'a, str>,
}

#[derive(Debug, Deserialize)]
pub struct DidOpenParams<'a> {
    #[serde(borrow, rename = "textDocument")]
    pub text_document: TextDocumentItem<'a>,
}

#[derive(Debug, Deserialize)]
pub struct DidSaveParams<'a> {
    #[serde(borrow, rename = "textDocument")]
    pub text_document: TextDocumentIdentifier<'a>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContentChange<'a> {
    #[serde(default)]
    pub range: Range,
    #[serde(rename = "rangeLength", default)]
    pub range_length: u32,
    #[serde(borrow)]
    pub text: Cow<'a, str>,
}

#[derive(Debug, Deserialize)]
pub struct DidChangeParams<'a> {
    #[serde(borrow, rename = "textDocument")]
    pub text_document: TextDocumentIdentifier<'a>,
    #[serde(borrow, rename = "contentChanges", default)]
    pub content_changes: Vec<ContentChange<'a>>,
}

#[derive(Debug, Deserialize)]
pub struct PositionParams<'a> {
    #[serde(borrow, rename = "textDocument")]
    pub text_document: TextDocumentIdentifier<'a>,
    #[serde(default)]
    pub position: Position,
}

#[derive(Debug, Deserialize)]
pub struct ExecuteCommandParams<'a> {
    #[serde(borrow, default)]
    pub command: Cow<'a, str>,
    #[serde(default)]
    pub arguments: Vec<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Results (01 §2.3, §3.1)
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

// ---------------------------------------------------------------------------
// initialize response (01 §3.1 — exact shape)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct InitializeResult {
    pub capabilities: ServerCapabilities,
    #[serde(rename = "serverInfo")]
    pub server_info: ServerInfo,
}

#[derive(Debug, Serialize)]
pub struct ServerCapabilities {
    #[serde(rename = "textDocumentSync")]
    pub text_document_sync: i32,
    #[serde(rename = "hoverProvider")]
    pub hover_provider: bool,
    #[serde(rename = "definitionProvider")]
    pub definition_provider: bool,
    #[serde(rename = "completionProvider")]
    pub completion_provider: serde_json::Map<String, serde_json::Value>,
    #[serde(rename = "executeCommandProvider")]
    pub execute_command_provider: ExecuteCommandOptions,
    // Post-parity additive capabilities (FEATURE_REQUESTS.md, 01 §3.1 note).
    #[serde(rename = "referencesProvider")]
    pub references_provider: bool,
    #[serde(rename = "documentSymbolProvider")]
    pub document_symbol_provider: bool,
    #[serde(rename = "workspaceSymbolProvider")]
    pub workspace_symbol_provider: bool,
    #[serde(rename = "documentHighlightProvider")]
    pub document_highlight_provider: bool,
    #[serde(rename = "foldingRangeProvider")]
    pub folding_range_provider: bool,
    #[serde(rename = "selectionRangeProvider")]
    pub selection_range_provider: bool,
    #[serde(rename = "semanticTokensProvider")]
    pub semantic_tokens_provider: serde_json::Value,
    #[serde(rename = "signatureHelpProvider")]
    pub signature_help_provider: serde_json::Value,
    #[serde(rename = "diagnosticProvider")]
    pub diagnostic_provider: serde_json::Value,
}

/// Semantic token legend (order defines the type indices in the encoding).
pub const SEMANTIC_TOKEN_TYPES: &[&str] = &[
    "keyword",
    "function",
    "macro",
    "variable",
    "string",
    "number",
    "operator",
    "namespace",
];

#[derive(Debug, Serialize)]
pub struct ExecuteCommandOptions {
    pub commands: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct ServerInfo {
    pub name: &'static str,
    pub version: String,
}

pub fn initialize_response(id: i64, version: &str) -> ResultResponse<InitializeResult> {
    ResultResponse::new(
        id,
        InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: 2,
                hover_provider: true,
                definition_provider: true,
                completion_provider: {
                    let mut map = serde_json::Map::new();
                    map.insert("resolveProvider".into(), serde_json::Value::Bool(true));
                    map
                },
                execute_command_provider: ExecuteCommandOptions {
                    commands: vec!["dbt.goToSchema"],
                },
                references_provider: true,
                document_symbol_provider: true,
                workspace_symbol_provider: true,
                document_highlight_provider: true,
                folding_range_provider: true,
                selection_range_provider: true,
                semantic_tokens_provider: serde_json::json!({
                    "legend": { "tokenTypes": SEMANTIC_TOKEN_TYPES, "tokenModifiers": [] },
                    "full": true,
                }),
                signature_help_provider: serde_json::json!({
                    "triggerCharacters": ["(", ","],
                }),
                diagnostic_provider: serde_json::json!({
                    "interFileDependencies": false,
                    "workspaceDiagnostics": false,
                }),
            },
            server_info: ServerInfo {
                name: "dbt-language-server",
                version: version.to_string(),
            },
        },
    )
}

// ---------------------------------------------------------------------------
// Post-parity additions (FEATURE_REQUESTS.md) — additive beyond the Go shape.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub jsonrpc: &'static str,
    pub id: Option<i64>,
    pub error: ResponseError,
}

#[derive(Debug, Serialize)]
pub struct ResponseError {
    pub code: i32,
    pub message: String,
}

pub mod error_codes {
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
}

impl ErrorResponse {
    pub fn new(id: i64, code: i32, message: impl Into<String>) -> Self {
        ErrorResponse {
            jsonrpc: "2.0",
            id: Some(id),
            error: ResponseError {
                code,
                message: message.into(),
            },
        }
    }
}

/// Probe for an `id` without knowing the message type (error responses are
/// sent only for requests, never notifications).
#[derive(Debug, Deserialize)]
pub struct MaybeId {
    #[serde(default)]
    pub id: Option<i64>,
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

#[derive(Debug, Deserialize)]
pub struct SelectionRangeParams<'a> {
    #[serde(borrow, rename = "textDocument")]
    pub text_document: TextDocumentIdentifier<'a>,
    #[serde(default)]
    pub positions: Vec<Position>,
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

#[derive(Debug, Deserialize)]
pub struct WorkspaceSymbolParams<'a> {
    #[serde(borrow, default)]
    pub query: Option<Cow<'a, str>>,
}
