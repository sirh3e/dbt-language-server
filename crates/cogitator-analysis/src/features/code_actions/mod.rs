//! textDocument/codeAction: the dbt command set at the cursor, served as
//! *command-only* actions so every stock LSP editor gets dbt workflows
//! with zero client code. Only navigating/side-effecting commands are
//! offered — payload-returning ones (dbt.modelGraph, dbt.columnLineage…)
//! make dishonest actions because clients drop executeCommand results.

pub(crate) mod convert_relation;
pub(crate) mod declare_missing;
pub(crate) mod define_missing;
pub(crate) mod extract_cte;
mod extract_cte_yaml;
pub(crate) mod governance_fixes;
pub(crate) mod inherit_docs;
pub(crate) mod inline_model;
mod quickfix;
mod raw_description;
pub(crate) mod refactors;
pub(crate) mod split_model;
pub(crate) mod staging_render;
pub(crate) mod staging_scaffold;
pub(crate) mod unique_test;
pub(crate) mod unit_test_scaffold;
pub(crate) mod yaml_columns;
pub(crate) mod yaml_fixes;
pub(crate) mod yaml_text;
pub(crate) mod yaml_unproduced;

use cogitator_lsp::{CodeAction, Command, Diagnostic, Range};
use cogitator_parser::TokenKind;
use serde::Serialize;

use super::lenses::{model_commands, CommandArgument};
use super::{file_uri, DbtContext, Document};

/// One profile×target pair offered as a "Switch target" action in the
/// active profiles.yml (assembled by the shell — fs stays out of here).
#[derive(Debug)]
pub struct TargetChoice {
    pub profile: String,
    pub target: String,
    pub active: bool,
}

#[derive(Debug, Serialize)]
struct SetTargetArgument {
    target: String,
    profile: String,
}

pub fn code_actions(
    ctx: &DbtContext,
    doc: Option<&Document>,
    uri: &str,
    range: Range,
    only: Option<&[String]>,
    targets: Option<&[TargetChoice]>,
    context_diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    // the active profiles.yml: target switching is the only action set
    // that makes sense there (works even in editors without pickers)
    let mut actions = if let Some(choices) = targets {
        choices.iter().map(target_action).collect()
    } else {
        let mut actions = Vec::new();
        if let Some(doc) = doc {
            actions.extend(ref_target_action(ctx, doc, range).map(CodeAction::at_cursor));
        }
        actions.extend(
            model_commands(ctx, uri)
                .into_iter()
                .map(|(title, command)| action(title, command, uri.to_string())),
        );
        if actions.is_empty() {
            // the entry under the cursor, of the several a YAML file holds
            actions.extend(
                yaml_entry_actions(ctx, uri, range)
                    .into_iter()
                    .map(CodeAction::at_cursor),
            );
        }
        actions
    };
    // quickfixes first: a fix beats a workflow command in every menu
    let mut all: Vec<CodeAction> = quickfix::quickfixes(uri, context_diagnostics)
        .into_iter()
        .map(CodeAction::at_cursor)
        .collect();
    all.extend(deps_action(ctx, uri, context_diagnostics));
    all.extend(quickfix::fix_all(uri, context_diagnostics, only));
    all.append(&mut actions);
    // kind filtering (01 §6.2): kind-less command actions drop out of a
    // kind-filtered request; quickfixes match by prefix
    all.retain(|a| a.allowed_by(only));
    all
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
/// The model entry at (or nearest above) the cursor in THIS
/// properties file — shared by the jump actions and the YAML fixes.
pub fn model_entry_at<'a>(
    ctx: &'a DbtContext,
    uri: &str,
    range: Range,
) -> Option<(&'a String, &'a crate::context::ModelDetails)> {
    let path = cogitator_lsp::uri_to_fs_path(uri).unwrap_or_else(|| uri.to_string());
    let path = std::path::PathBuf::from(path);
    ctx.model_detail_map
        .iter()
        // Path equality compares components, so `/` and `\` match
        .filter(|(_, d)| d.schema_uri.as_deref() == Some(path.as_path()))
        .filter(|(_, d)| d.schema_range.start.line <= range.start.line)
        .max_by_key(|(name, d)| (d.schema_range.start.line, name.as_str()))
}

fn yaml_entry_actions(ctx: &DbtContext, uri: &str, range: Range) -> Vec<CodeAction> {
    let Some((name, details)) = model_entry_at(ctx, uri, range) else {
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

/// packages.yml naming packages that are not installed: "Run dbt deps",
/// a quickfix that runs `dbt.deps` (the edit is dbt's to make). Tied to
/// the client's package-not-installed diagnostics where it sent them.
fn deps_action(ctx: &DbtContext, uri: &str, context: &[Diagnostic]) -> Option<CodeAction> {
    let missing = super::diagnostics::packages_diagnostics(ctx, uri);
    if missing.is_empty() {
        return None;
    }
    let tied: Vec<Diagnostic> = context
        .iter()
        .filter(|d| d.code == "package-not-installed")
        .cloned()
        .collect();
    let title = format!("Run dbt deps ({} package(s) missing)", missing.len());
    let arg = serde_json::to_value(CommandArgument {
        uri: uri.to_string(),
    })
    .expect("action argument serializes");
    let mut action = CodeAction::command_only(
        title.clone(),
        Command {
            title,
            command: "dbt.deps".to_string(),
            arguments: vec![arg],
        },
    );
    action.kind = Some("quickfix");
    action.diagnostics = (!tied.is_empty()).then_some(tied);
    Some(action)
}

fn target_action(choice: &TargetChoice) -> CodeAction {
    let title = format!(
        "Switch target: {}/{}{}",
        choice.profile,
        choice.target,
        if choice.active { " (active)" } else { "" }
    );
    let arg = serde_json::to_value(SetTargetArgument {
        target: choice.target.clone(),
        profile: choice.profile.clone(),
    })
    .expect("action argument serializes");
    CodeAction::command_only(
        title.clone(),
        Command {
            title,
            command: "dbt.setTarget".to_string(),
            arguments: vec![arg],
        },
    )
}

fn action(title: String, command: &'static str, uri: String) -> CodeAction {
    let arg = serde_json::to_value(CommandArgument { uri }).expect("action argument serializes");
    CodeAction::command_only(
        title.clone(),
        Command {
            title,
            command: command.to_string(),
            arguments: vec![arg],
        },
    )
}
