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
    /// Kept as raw JSON: the spec allows `any`, so a scalar or an unknown
    /// shape must not fail the whole initialize.
    #[serde(rename = "initializationOptions", default)]
    pub initialization_options: Option<serde_json::Value>,
    /// Client capabilities, kept raw: only a few flags are consulted.
    #[serde(default)]
    pub capabilities: Option<serde_json::Value>,
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

    /// window.workDoneProgress from the client capabilities ($/progress).
    pub fn work_done_progress(&self) -> bool {
        self.capabilities
            .as_ref()
            .and_then(|c| c.pointer("/window/workDoneProgress"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    fn init_option_str(&self, keys: &[&str]) -> Option<&str> {
        let opts = self.initialization_options.as_ref()?;
        keys.iter()
            .find_map(|key| opts.get(*key))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
    }

    /// `initializationOptions.profilesDir` (or `profiles_dir`), when the
    /// client sent one.
    pub fn profiles_dir(&self) -> Option<&str> {
        self.init_option_str(&["profilesDir", "profiles_dir"])
    }

    /// `initializationOptions.profile`: the active profile override.
    pub fn profile(&self) -> Option<&str> {
        self.init_option_str(&["profile"])
    }

    /// `initializationOptions.target`: the active target override.
    pub fn target(&self) -> Option<&str> {
        self.init_option_str(&["target"])
    }

    /// `initializationOptions.profilesAllow` (or `profiles_allow`): extra
    /// profiles the server may read besides the project's own.
    pub fn profiles_allow(&self) -> Vec<String> {
        self.initialization_options
            .as_ref()
            .and_then(|opts| {
                opts.get("profilesAllow")
                    .or_else(|| opts.get("profiles_allow"))
            })
            .and_then(|v| v.as_array())
            .map(|list| {
                list.iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// `file://` URI → filesystem path (percent-decoded). Windows drive-letter
/// URIs (`file:///C:/…`; VS Code sends `file:///c%3A/…`) decode to `C:/…` —
/// `Path` accepts forward slashes on Windows.
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
    String::from_utf8(out).ok().map(strip_drive_slash)
}

/// `/C:/work` (the decoded path of a Windows drive URI) → `C:/work`;
/// anything else is returned unchanged.
fn strip_drive_slash(path: String) -> String {
    let b = path.as_bytes();
    let is_drive = b.len() >= 3
        && b[0] == b'/'
        && b[1].is_ascii_alphabetic()
        && b[2] == b':'
        && (b.len() == 3 || b[3] == b'/');
    if is_drive {
        path[1..].to_string()
    } else {
        path
    }
}

/// Filesystem path → `file://` URI: backslashes become `/`, a Windows drive
/// path gets its leading slash (`C:\w` → `file:///C:/w`), and bytes outside
/// the URI path character set are percent-encoded.
pub fn fs_path_to_uri(path: &std::path::Path) -> String {
    use std::fmt::Write;

    let raw = path.to_string_lossy().replace('\\', "/");
    let mut out = String::with_capacity(raw.len() + 8);
    out.push_str("file://");
    if !raw.starts_with('/') {
        out.push('/');
    }
    for byte in raw.bytes() {
        match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'.'
            | b'_'
            | b'~'
            | b'/'
            | b':'
            | b'+'
            | b'@' => out.push(byte as char),
            _ => {
                let _ = write!(out, "%{byte:02X}");
            }
        }
    }
    out
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
    use super::{fs_path_to_uri, uri_to_fs_path};

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

    #[test]
    fn initialization_options_profiles_dir() {
        let params: super::InitializeParams = serde_json::from_str(
            r#"{"rootUri":"file:///w","initializationOptions":{"profilesDir":"/x/profiles"}}"#,
        )
        .unwrap();
        assert_eq!(params.profiles_dir(), Some("/x/profiles"));
        // a scalar `initializationOptions` must not fail the initialize parse
        let params: super::InitializeParams =
            serde_json::from_str(r#"{"initializationOptions":"nonsense"}"#).unwrap();
        assert_eq!(params.profiles_dir(), None);
    }

    #[test]
    fn windows_drive_uris() {
        // VS Code on Windows percent-encodes the drive colon
        assert_eq!(
            uri_to_fs_path("file:///c%3A/work/proj"),
            Some("c:/work/proj".into())
        );
        assert_eq!(
            uri_to_fs_path("file:///C:/work/proj"),
            Some("C:/work/proj".into())
        );
        // a bare drive root, and a non-drive path that merely looks close
        assert_eq!(uri_to_fs_path("file:///c%3A"), Some("c:".into()));
        assert_eq!(uri_to_fs_path("file:///cx/y"), Some("/cx/y".into()));
    }

    #[test]
    fn path_encoding() {
        use std::path::Path;
        assert_eq!(fs_path_to_uri(Path::new("/w/proj")), "file:///w/proj");
        assert_eq!(
            fs_path_to_uri(Path::new("/w/my proj")),
            "file:///w/my%20proj"
        );
        assert_eq!(
            fs_path_to_uri(Path::new(r"C:\work\proj")),
            "file:///C:/work/proj"
        );
        // round-trips through the decoder
        assert_eq!(
            uri_to_fs_path(&fs_path_to_uri(Path::new("C:/a b/c.sql"))),
            Some("C:/a b/c.sql".into())
        );
    }
}
