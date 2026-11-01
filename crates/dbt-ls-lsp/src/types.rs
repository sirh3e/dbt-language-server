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
}

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
                completion_provider: serde_json::Map::new(),
                execute_command_provider: ExecuteCommandOptions {
                    commands: vec!["dbt.goToSchema"],
                },
            },
            server_info: ServerInfo {
                name: "dbt-language-server",
                version: version.to_string(),
            },
        },
    )
}
