//! The selector engine's shell: builds the project's universe from the
//! open documents' text and the files on disk, off the dispatcher task,
//! and answers `dbt.select`, hover, lenses, completion, pull diagnostics
//! and `workspace/symbol` from it (`cogitator_analysis::selectors`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cogitator_analysis::features::selectors_yml;
use cogitator_analysis::selectors::{self, Definition, Universe};
use cogitator_analysis::DbtContext;
use cogitator_lsp::{CodeLens, CompletionItem, Diagnostic, Position};
use serde_json::{Map, Value};

use crate::server::reply::Reply;

/// Open documents by path: their unsaved text wins over the disk.
pub struct Open(HashMap<PathBuf, Arc<str>>);

impl Open {
    pub fn of(texts: Vec<(String, Arc<str>)>) -> Self {
        Open(
            texts
                .into_iter()
                .filter_map(|(uri, text)| {
                    Some((PathBuf::from(cogitator_lsp::uri_to_fs_path(&uri)?), text))
                })
                .collect(),
        )
    }

    fn read(&self, path: &Path) -> Option<String> {
        match self.0.get(path) {
            Some(text) => Some(text.to_string()),
            None => std::fs::read_to_string(path).ok(),
        }
    }
}

/// The universe and the selectors.yml definitions (the open buffer's
/// when selectors.yml is open, else the context's).
fn engine(ctx: &DbtContext, open: &Open) -> (Universe, Vec<Definition>) {
    let read = |path: &Path| open.read(path);
    let universe = selectors::universe(ctx, &read);
    let defs = ctx
        .project_root
        .as_deref()
        .and_then(|root| open.0.get(&root.join("selectors.yml")))
        .map(|text| selectors::yaml::parse(text).definitions)
        .unwrap_or_else(|| ctx.selectors.definitions.clone());
    (universe, defs)
}

/// `dbt.select {selector}`.
pub fn select_reply(ctx: &DbtContext, open: Open, arg: &Map<String, Value>) -> Reply {
    let Some(selector) = arg.get("selector").and_then(Value::as_str) else {
        return Reply::error(
            cogitator_lsp::error_codes::INVALID_PARAMS,
            "dbt.select takes {selector}: a dbt node selector, e.g. `tag:nightly` or `selector:nightly`.".into(),
        );
    };
    let (universe, defs) = engine(ctx, &open);
    match selectors_yml::select_command(&universe, &defs, selector) {
        Ok(answer) => Reply::result(&answer),
        Err(message) => Reply::error(cogitator_lsp::error_codes::INVALID_PARAMS, message),
    }
}

pub fn hover(ctx: &DbtContext, open: Open, text: &str, pos: Position) -> Option<String> {
    let (universe, _) = engine(ctx, &open);
    selectors_yml::selectors_hover(text, pos, &universe)
}

pub fn lenses(ctx: &DbtContext, open: Open, uri: &str, text: &str) -> Vec<CodeLens> {
    let (universe, _) = engine(ctx, &open);
    selectors_yml::selectors_lenses(uri, text, &universe)
}

pub fn completion(
    ctx: &DbtContext,
    open: Open,
    uri: &str,
    text: &str,
    pos: Position,
) -> Vec<CompletionItem> {
    let (universe, mut defs) = engine(ctx, &open);
    if selectors_yml::is_selectors_file(uri) {
        defs = selectors::yaml::parse(text).definitions;
    }
    selectors_yml::selector_completion(uri, text, pos, &universe, &defs).unwrap_or_default()
}

/// The pull path's extra: definitions that match nothing.
pub fn unmatched(ctx: &DbtContext, open: Open, text: &str) -> Vec<Diagnostic> {
    let (universe, _) = engine(ctx, &open);
    selectors_yml::unmatched_selector_diagnostics(text, &universe)
}

/// `workspace/symbol` over the full universe.
pub fn workspace_symbols(ctx: &DbtContext, open: Open, query: &str) -> Reply {
    let (universe, _) = engine(ctx, &open);
    Reply::result(&cogitator_analysis::features::workspace_symbols_in(
        ctx, &universe, query,
    ))
}
