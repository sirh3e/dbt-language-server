//! initialize response: capabilities + server info (01 §3.1; post-parity
//! capabilities are additive — see the note in the spec).

use serde::Serialize;

use super::results::ResultResponse;

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
    #[serde(rename = "renameProvider")]
    pub rename_provider: bool,
    #[serde(rename = "codeLensProvider")]
    pub code_lens_provider: serde_json::Value,
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
                    commands: vec![
                        "dbt.goToSchema",
                        "dbt.columnLineage",
                        "dbt.openSchema",
                        "dbt.showLineage",
                        "dbt.run",
                        "dbt.test",
                        "dbt.compile",
                        "dbt.modelGraph",
                        "dbt.profiles",
                        "dbt.setTarget",
                        "dbt.extractModelYaml",
                        "dbt.modelTests",
                        "dbt.goToTests",
                        "dbt.status",
                    ],
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
                rename_provider: true,
                code_lens_provider: serde_json::json!({ "resolveProvider": false }),
            },
            server_info: ServerInfo {
                name: "cogitator",
                version: version.to_string(),
            },
        },
    )
}
