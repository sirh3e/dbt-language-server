//! Incoming request/notification params — borrow from the frame buffer
//! where serde allows (08 §1.2).

use std::borrow::Cow;

use serde::Deserialize;

use super::core::{Position, Range};

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

#[derive(Debug, Deserialize)]
pub struct SelectionRangeParams<'a> {
    #[serde(borrow, rename = "textDocument")]
    pub text_document: TextDocumentIdentifier<'a>,
    #[serde(default)]
    pub positions: Vec<Position>,
}

#[derive(Debug, Deserialize)]
pub struct WorkspaceSymbolParams<'a> {
    #[serde(borrow, default)]
    pub query: Option<Cow<'a, str>>,
}
