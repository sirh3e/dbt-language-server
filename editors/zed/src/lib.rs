//! Zed extension for the cogitator dbt language server.
//!
//! Finds the `cogitator` binary (settings override first, then PATH) and
//! passes `lsp.cogitator.initialization_options` through, so Zed users get
//! the same profiles support as the other editors:
//!
//! ```json
//! { "lsp": { "cogitator": {
//!     "binary": { "path": "/opt/cogitator", "arguments": ["--fusion"] },
//!     "initialization_options": { "profilesDir": "/data/dbt-profiles" }
//! } } }
//! ```

use zed_extension_api::{self as zed, settings::LspSettings, LanguageServerId, Result};

struct CogitatorExtension;

impl zed::Extension for CogitatorExtension {
    fn new() -> Self {
        CogitatorExtension
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let settings = LspSettings::for_worktree("cogitator", worktree).ok();
        let binary = settings.as_ref().and_then(|s| s.binary.as_ref());
        let command = binary
            .and_then(|b| b.path.clone())
            .or_else(|| worktree.which("cogitator"))
            .ok_or_else(|| {
                "cogitator not found.\n\
                 Reason: no `lsp.cogitator.binary.path` setting and no \
                 `cogitator` on PATH.\n\
                 Solution: download a release asset \
                 (cogitator-{linux-amd64,darwin-amd64,darwin-arm64,windows-amd64.exe}) \
                 and put it on PATH, or point the setting at it."
                    .to_string()
            })?;
        let args = binary
            .and_then(|b| b.arguments.clone())
            .unwrap_or_default();
        Ok(zed::Command {
            command,
            args,
            env: worktree.shell_env(),
        })
    }

    fn language_server_initialization_options(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<zed::serde_json::Value>> {
        Ok(LspSettings::for_worktree("cogitator", worktree)
            .ok()
            .and_then(|settings| settings.initialization_options))
    }
}

zed::register_extension!(CogitatorExtension);
