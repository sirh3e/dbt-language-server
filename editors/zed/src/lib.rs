//! Zed extension for the cogitator dbt language server.
//!
//! Finds the `cogitator` binary (settings override first, then PATH) and
//! passes `lsp.cogitator.initialization_options` through, so Zed users get
//! the same profiles support as the other editors. It also asks the server
//! for its dialogs (`"dialogs": true`, unless the settings say `false`):
//! an extension cannot add commands, pickers or panels, so the server
//! offers the dbt menu as the code action "dbt…" and walks it with
//! `window/showMessageRequest` prompts, opening each answer as a document —
//! what Neovim's `:Dbt` menu and its pickers do.
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
        let options = LspSettings::for_worktree("cogitator", worktree)
            .ok()
            .and_then(|settings| settings.initialization_options);
        Ok(Some(with_dialogs(options)))
    }
}

/// The settings' initialization options with `"dialogs": true` added,
/// unless they set `dialogs` themselves (`false` turns the menu off).
fn with_dialogs(options: Option<zed::serde_json::Value>) -> zed::serde_json::Value {
    let mut options = match options {
        Some(zed::serde_json::Value::Object(map)) => map,
        _ => zed::serde_json::Map::new(),
    };
    options
        .entry("dialogs")
        .or_insert(zed::serde_json::Value::Bool(true));
    zed::serde_json::Value::Object(options)
}

#[cfg(test)]
mod tests {
    use super::with_dialogs;
    use zed_extension_api::serde_json::json;

    #[test]
    fn the_dialogs_are_asked_for_unless_turned_off() {
        assert_eq!(with_dialogs(None), json!({ "dialogs": true }));
        assert_eq!(
            with_dialogs(Some(json!({ "profilesDir": "/p" }))),
            json!({ "profilesDir": "/p", "dialogs": true })
        );
        assert_eq!(
            with_dialogs(Some(json!({ "dialogs": false }))),
            json!({ "dialogs": false })
        );
    }
}

zed::register_extension!(CogitatorExtension);
