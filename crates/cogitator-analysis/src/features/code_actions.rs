//! textDocument/codeAction: the dbt command set at the cursor, served as
//! *command-only* actions so every stock LSP editor gets dbt workflows
//! with zero client code. Only navigating/side-effecting commands are
//! offered — payload-returning ones (dbt.modelGraph, dbt.columnLineage…)
//! make dishonest actions because clients drop executeCommand results.

use cogitator_lsp::{CodeAction, Command, Range};
use cogitator_parser::TokenKind;

use super::lenses::{model_commands, CommandArgument};
use super::{file_uri, DbtContext, Document};

pub fn code_actions(
    ctx: &DbtContext,
    doc: Option<&Document>,
    uri: &str,
    range: Range,
    only: Option<&[String]>,
) -> Vec<CodeAction> {
    // our actions are kind-less, so a client filtering for specific kinds
    // (quickfix, refactor…) honestly gets nothing
    if only.is_some_and(|kinds| !kinds.is_empty()) {
        return Vec::new();
    }
    let mut actions = Vec::new();
    if let Some(doc) = doc {
        actions.extend(ref_target_action(ctx, doc, range));
    }
    actions.extend(
        model_commands(ctx, uri)
            .into_iter()
            .map(|(title, command)| action(title, command, uri.to_string())),
    );
    if actions.is_empty() {
        actions.extend(yaml_entry_actions(ctx, uri, range));
    }
    actions
}

/// On a `ref('x')` token: open x's schema entry (when it has one) without
/// leaving the current file first.
fn ref_target_action(ctx: &DbtContext, doc: &Document, range: Range) -> Option<CodeAction> {
    let parsed = &doc.parsed;
    let idx = parsed.find_token_at_cursor(range.start.line, range.start.character)?;
    let token = &parsed.tokens[idx];
    if token.kind != TokenKind::Ref {
        return None;
    }
    let name = parsed.literal(token);
    let details = ctx.model_detail_map.get(name)?;
    details.schema_uri.as_ref()?;
    Some(action(
        format!("Open schema of '{name}'"),
        "dbt.openSchema",
        file_uri(&details.uri),
    ))
}

/// In a properties YAML: the model entry at (or nearest above) the cursor
/// offers jumps to its file and its tests.
fn yaml_entry_actions(ctx: &DbtContext, uri: &str, range: Range) -> Vec<CodeAction> {
    let path = cogitator_lsp::uri_to_fs_path(uri).unwrap_or_else(|| uri.to_string());
    let path = std::path::Path::new(&path);
    let entry = ctx
        .model_detail_map
        .iter()
        // Path equality compares components, so `/` and `\` match
        .filter(|(_, d)| d.schema_uri.as_deref() == Some(path))
        .filter(|(_, d)| d.schema_range.start.line <= range.start.line)
        .max_by_key(|(name, d)| (d.schema_range.start.line, name.as_str()));
    let Some((name, details)) = entry else {
        return Vec::new();
    };
    if details.uri.as_os_str().is_empty() {
        return Vec::new();
    }
    let model_uri = file_uri(&details.uri);
    vec![
        action(
            format!("Open model '{name}'"),
            "dbt.openModel",
            model_uri.clone(),
        ),
        action(
            format!("Go to tests of '{name}'"),
            "dbt.goToTests",
            model_uri,
        ),
    ]
}

fn action(title: String, command: &'static str, uri: String) -> CodeAction {
    let arg = serde_json::to_value(CommandArgument { uri }).expect("action argument serializes");
    CodeAction {
        command: Command {
            title: title.clone(),
            command: command.to_string(),
            arguments: vec![arg],
        },
        title,
    }
}
