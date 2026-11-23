//! workspace file-operation wire types (LSP 3.16): the server's
//! `workspace.fileOperations` capability and the params of
//! will/didCreate, will/didRename and will/didDelete files.

use std::borrow::Cow;

use serde::{Deserialize, Serialize};

/// The project resource directories dbt reads by default — what the
/// file-operation filters watch (a project with custom `*-paths` is
/// still served: the handlers decide from the context, not the glob).
pub const RESOURCE_DIRS: &[&str] = &[
    "models",
    "macros",
    "snapshots",
    "seeds",
    "tests",
    "analyses",
];

/// The file kinds a dbt project is made of, in those directories.
pub const RESOURCE_EXTENSIONS: &[&str] = &["sql", "py", "yml", "yaml", "csv"];

#[derive(Debug, Clone, Serialize)]
pub struct FileOperationPattern {
    pub glob: String,
    /// `file` or `folder`.
    pub matches: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileOperationFilter {
    pub scheme: &'static str,
    pub pattern: FileOperationPattern,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileOperationRegistrationOptions {
    pub filters: Vec<FileOperationFilter>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileOperationsServerCapabilities {
    #[serde(rename = "didCreate")]
    pub did_create: FileOperationRegistrationOptions,
    #[serde(rename = "willRename")]
    pub will_rename: FileOperationRegistrationOptions,
    #[serde(rename = "didRename")]
    pub did_rename: FileOperationRegistrationOptions,
    #[serde(rename = "willDelete")]
    pub will_delete: FileOperationRegistrationOptions,
    #[serde(rename = "didDelete")]
    pub did_delete: FileOperationRegistrationOptions,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceServerCapabilities {
    #[serde(rename = "fileOperations")]
    pub file_operations: FileOperationsServerCapabilities,
}

/// The resource files of the default directories; with `folders`, the
/// directories below them too (a moved or deleted folder of models).
pub fn resource_filters(folders: bool) -> FileOperationRegistrationOptions {
    let dirs = RESOURCE_DIRS.join(",");
    let mut filters = vec![FileOperationFilter {
        scheme: "file",
        pattern: FileOperationPattern {
            glob: format!("**/{{{dirs}}}/**/*.{{{}}}", RESOURCE_EXTENSIONS.join(",")),
            matches: "file",
        },
    }];
    if folders {
        filters.push(FileOperationFilter {
            scheme: "file",
            pattern: FileOperationPattern {
                glob: format!("**/{{{dirs}}}/**"),
                matches: "folder",
            },
        });
    }
    FileOperationRegistrationOptions { filters }
}

pub fn workspace_capabilities() -> WorkspaceServerCapabilities {
    WorkspaceServerCapabilities {
        file_operations: FileOperationsServerCapabilities {
            did_create: resource_filters(false),
            will_rename: resource_filters(true),
            did_rename: resource_filters(true),
            will_delete: resource_filters(true),
            did_delete: resource_filters(true),
        },
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileRename<'a> {
    #[serde(rename = "oldUri", borrow)]
    pub old_uri: Cow<'a, str>,
    #[serde(rename = "newUri", borrow)]
    pub new_uri: Cow<'a, str>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RenameFilesParams<'a> {
    #[serde(borrow)]
    pub files: Vec<FileRename<'a>>,
}

/// One file of a create or delete operation.
#[derive(Debug, Clone, Deserialize)]
pub struct FileUri<'a> {
    #[serde(borrow)]
    pub uri: Cow<'a, str>,
}

/// The params of will/didCreateFiles and will/didDeleteFiles.
#[derive(Debug, Clone, Deserialize)]
pub struct FileUrisParams<'a> {
    #[serde(borrow)]
    pub files: Vec<FileUri<'a>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_filters_cover_every_resource_dir_and_kind() {
        let json = serde_json::to_value(workspace_capabilities()).unwrap();
        let ops = &json["fileOperations"];
        for (op, folder) in [
            ("didCreate", false),
            ("willRename", true),
            ("didRename", true),
            ("willDelete", true),
            ("didDelete", true),
        ] {
            let filters = ops[op]["filters"].as_array().unwrap();
            assert_eq!(filters.len(), if folder { 2 } else { 1 }, "{op}");
            let glob = filters[0]["pattern"]["glob"].as_str().unwrap();
            assert_eq!(
                glob,
                "**/{models,macros,snapshots,seeds,tests,analyses}/**/*.{sql,py,yml,yaml,csv}"
            );
            assert_eq!(filters[0]["scheme"], "file");
            assert_eq!(filters[0]["pattern"]["matches"], "file");
        }
    }

    #[test]
    fn rename_params_borrow_their_uris() {
        let raw = r#"{"files":[{"oldUri":"file:///a/x.sql","newUri":"file:///a/y.sql"}]}"#;
        let params: RenameFilesParams<'_> = serde_json::from_str(raw).unwrap();
        assert_eq!(params.files[0].old_uri, "file:///a/x.sql");
        assert_eq!(params.files[0].new_uri, "file:///a/y.sql");
        let raw = r#"{"files":[{"uri":"file:///a/x.sql"}]}"#;
        let params: FileUrisParams<'_> = serde_json::from_str(raw).unwrap();
        assert_eq!(params.files[0].uri, "file:///a/x.sql");
    }
}
