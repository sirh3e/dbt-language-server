//! textDocument/codeAction wire types. The server serves *command-only*
//! actions (its dbt.* commands at the cursor, no edits), so the response
//! type carries a title and a Command and nothing else — kinds, edits and
//! resolve do not exist here.

use std::borrow::Cow;

use serde::Deserialize;
use serde::Serialize;

use super::core::Range;
use super::params::TextDocumentIdentifier;
use super::results::Command;

#[derive(Debug, Deserialize)]
pub struct CodeActionParams<'a> {
    #[serde(borrow, rename = "textDocument")]
    pub text_document: TextDocumentIdentifier<'a>,
    #[serde(default)]
    pub range: Range,
    #[serde(default)]
    pub context: Option<CodeActionContext<'a>>,
}

/// Only `only` matters to us; the diagnostics member is deliberately
/// ignored (none of our actions are quickfixes).
#[derive(Debug, Default, Deserialize)]
pub struct CodeActionContext<'a> {
    #[serde(borrow, default)]
    pub only: Option<Vec<Cow<'a, str>>>,
}

/// A command-only CodeAction: clients run `command` through
/// workspace/executeCommand; navigation then arrives server-initiated
/// (window/showDocument, workspace/applyEdit, messages).
#[derive(Debug, Serialize)]
pub struct CodeAction {
    pub title: String,
    pub command: Command,
}
