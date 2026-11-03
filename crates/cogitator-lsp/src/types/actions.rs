//! textDocument/codeAction wire types (01 §6.2): actions carry a kind,
//! context diagnostics and optionally a WorkspaceEdit, so quickfixes
//! ride the same response as the command-only dbt actions.

use std::borrow::Cow;

use serde::Deserialize;
use serde::Serialize;

use super::core::Range;
use super::params::TextDocumentIdentifier;
use super::results::{Command, Diagnostic};
use super::workspace::WorkspaceEdit;

#[derive(Debug, Deserialize)]
pub struct CodeActionParams<'a> {
    #[serde(borrow, rename = "textDocument")]
    pub text_document: TextDocumentIdentifier<'a>,
    #[serde(default)]
    pub range: Range,
    #[serde(default)]
    pub context: Option<CodeActionContext<'a>>,
}

/// `only` filters by kind prefix; `diagnostics` carry the typed `data`
/// payloads back for quickfix assembly (01 §6.1-6.2).
#[derive(Debug, Default, Deserialize)]
pub struct CodeActionContext<'a> {
    #[serde(borrow, default)]
    pub only: Option<Vec<Cow<'a, str>>>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
}

/// A CodeAction is either command-only (the dbt workflow set — clients
/// run `command` through workspace/executeCommand) or an edit-bearing
/// quickfix; unset members stay off the wire.
#[derive(Debug, Serialize)]
pub struct CodeAction {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<Vec<Diagnostic>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edit: Option<WorkspaceEdit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<Command>,
}

impl CodeAction {
    /// The classic kind-less dbt action: a command, nothing else.
    pub fn command_only(title: String, command: Command) -> Self {
        CodeAction {
            title,
            kind: None,
            diagnostics: None,
            edit: None,
            command: Some(command),
        }
    }

    /// An edit-bearing quickfix tied to the diagnostic it fixes.
    pub fn quickfix(title: String, diagnostic: Diagnostic, edit: WorkspaceEdit) -> Self {
        CodeAction {
            title,
            kind: Some("quickfix"),
            diagnostics: Some(vec![diagnostic]),
            edit: Some(edit),
            command: None,
        }
    }

    /// Does this action survive the client's `only` filter? Kind-less
    /// actions honestly drop out of any kind-filtered request.
    pub fn allowed_by(&self, only: Option<&[String]>) -> bool {
        let Some(kinds) = only.filter(|k| !k.is_empty()) else {
            return true;
        };
        let Some(kind) = self.kind else { return false };
        kinds
            .iter()
            .any(|k| k.is_empty() || kind.starts_with(k.as_str()))
    }
}
