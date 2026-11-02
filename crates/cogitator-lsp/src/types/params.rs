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
    #[serde(borrow, rename = "rootUri", default)]
    pub root_uri: Option<Cow<'a, str>>,
    #[serde(borrow, rename = "workspaceFolders", default)]
    pub workspace_folders: Option<Vec<WorkspaceFolder<'a>>>,
}

#[derive(Debug, Deserialize)]
pub struct WorkspaceFolder<'a> {
    #[serde(borrow)]
    pub uri: Cow<'a, str>,
    #[serde(borrow, default)]
    pub name: Option<Cow<'a, str>>,
}

impl InitializeParams<'_> {
    /// Workspace root as a filesystem path: workspaceFolders[0] wins, then
    /// rootUri, then the deprecated rootPath (interop fix — modern clients
    /// may omit rootPath entirely).
    pub fn workspace_root(&self) -> Option<String> {
        if let Some(folder) = self.workspace_folders.as_ref().and_then(|f| f.first()) {
            if let Some(path) = uri_to_fs_path(&folder.uri) {
                return Some(path);
            }
        }
        if let Some(uri) = &self.root_uri {
            if let Some(path) = uri_to_fs_path(uri) {
                return Some(path);
            }
        }
        self.root_path.as_ref().map(|p| p.to_string())
    }
}

/// `file://` URI → filesystem path (percent-decoded).
pub fn uri_to_fs_path(uri: &str) -> Option<String> {
    let rest = uri.strip_prefix("file://")?;
    // strip an authority (file://localhost/..., rare)
    let path = match rest.find('/') {
        Some(0) => rest,
        Some(idx) => &rest[idx..],
        None => return None,
    };
    let mut out = Vec::with_capacity(path.len());
    let bytes = path.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&path[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).ok()
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

#[derive(Debug, Deserialize)]
pub struct RenameParams<'a> {
    #[serde(borrow, rename = "textDocument")]
    pub text_document: TextDocumentIdentifier<'a>,
    #[serde(default)]
    pub position: Position,
    #[serde(borrow, rename = "newName", default)]
    pub new_name: Cow<'a, str>,
}

#[cfg(test)]
mod tests {
    use super::uri_to_fs_path;

    #[test]
    fn uri_decoding() {
        assert_eq!(uri_to_fs_path("file:///w/proj"), Some("/w/proj".into()));
        assert_eq!(
            uri_to_fs_path("file:///w/my%20proj"),
            Some("/w/my proj".into())
        );
        assert_eq!(uri_to_fs_path("file://localhost/w"), Some("/w".into()));
        assert_eq!(uri_to_fs_path("untitled:x"), None);
    }
}
