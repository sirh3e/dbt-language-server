//! Message envelopes, ids and error responses (01 §2.1 + post-parity).

use std::borrow::Cow;

use serde::{Deserialize, Serialize};

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
    /// LSP: the client cancelled the request ($/cancelRequest).
    pub const REQUEST_CANCELLED: i32 = -32800;
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
