//! dbt.incrementalDiff: what `is_incremental()` changes in an
//! incremental model. The model is rendered twice with the
//! dbt.expandJinja stubs — `is_incremental()` false (a full refresh, or
//! the first run) and true (every later run) — and the two SQL texts are
//! compared as a unified diff. Plus the "Compare incremental / full
//! refresh" code lens on incremental models. Pure: the shell assembles
//! the [`PreviewInputs`] and the manifest's materialization.

use cogitator_error::guidance::Guidance;
use cogitator_lsp::{CodeLens, Command, Range};
use minijinja::machinery::{tokenize, Token};
use minijinja::syntax::SyntaxConfig;
use serde::Serialize;
use similar::TextDiff;

use super::jinja_preview::{expand_jinja, PreviewInputs};
use super::{model_name_from_uri, DbtContext};

/// The dbt.incrementalDiff answer.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IncrementalDiff {
    pub model: String,
    /// Rendered with `is_incremental()` false.
    pub full_refresh: String,
    /// Rendered with `is_incremental()` true.
    pub incremental: String,
    /// Unified diff, full refresh → incremental; empty when identical.
    pub diff: String,
    pub identical: bool,
}

/// Renders `body` both ways and diffs the results.
pub fn incremental_diff(
    model: &str,
    body: &str,
    inputs: &mut PreviewInputs,
) -> Result<IncrementalDiff, Guidance> {
    inputs.is_incremental = false;
    let full_refresh = expand_jinja(body, inputs)?.sql;
    inputs.is_incremental = true;
    let incremental = expand_jinja(body, inputs)?.sql;
    let identical = full_refresh == incremental;
    let diff = if identical {
        String::new()
    } else {
        TextDiff::from_lines(&full_refresh, &incremental)
            .unified_diff()
            .context_radius(3)
            .header(
                &format!("{model} (full refresh)"),
                &format!("{model} (incremental)"),
            )
            .to_string()
    };
    Ok(IncrementalDiff {
        model: model.to_string(),
        full_refresh,
        incremental,
        diff,
        identical,
    })
}

/// The `materialized` a `config(...)` call in the file sets as a string
/// literal; the last call wins, as in dbt. Read from minijinja's own
/// tokens, so a template that does not tokenize answers `None`.
pub fn config_materialized(text: &str) -> Option<String> {
    let tokens: Vec<Token<'_>> = tokenize(text, false, SyntaxConfig::default())
        .map(|t| t.map(|(token, _)| token))
        .collect::<Result<_, _>>()
        .ok()?;
    let mut found = None;
    let mut i = 0;
    while i + 1 < tokens.len() {
        let opens = matches!(tokens[i], Token::Ident("config"))
            && matches!(tokens[i + 1], Token::ParenOpen);
        if !opens {
            i += 1;
            continue;
        }
        let mut depth = 0usize;
        let mut j = i + 1;
        while j < tokens.len() {
            match &tokens[j] {
                Token::ParenOpen | Token::BracketOpen | Token::BraceOpen => depth += 1,
                Token::ParenClose | Token::BracketClose | Token::BraceClose => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                Token::Ident("materialized") if depth == 1 => {
                    if let (Some(Token::Assign), Some(value)) =
                        (tokens.get(j + 1), tokens.get(j + 2))
                    {
                        match value {
                            Token::Str(s) => found = Some((*s).to_string()),
                            Token::String(s) => found = Some(s.to_string()),
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
            j += 1;
        }
        i = j + 1;
    }
    found
}

/// Is the model incremental? The file's own `config()` wins over what the
/// manifest records (dbt_project.yml and properties).
pub fn is_incremental_model(text: &str, manifest_materialized: Option<&str>) -> bool {
    config_materialized(text)
        .as_deref()
        .or(manifest_materialized)
        .is_some_and(|m| m == "incremental")
}

pub const INCREMENTAL_LENS_TITLE: &str = "Compare incremental / full refresh";

/// The lens on an incremental model. Its argument asks the server to
/// open the diff as a document (`show: true`), so it works in every
/// client without wrapping.
pub fn incremental_lens(
    ctx: &DbtContext,
    uri: &str,
    text: &str,
    manifest_materialized: Option<&str>,
) -> Option<CodeLens> {
    let model = model_name_from_uri(uri)?;
    if !ctx.model_detail_map.contains_key(&model) {
        return None;
    }
    is_incremental_model(text, manifest_materialized).then(|| CodeLens {
        range: Range::default(),
        command: Command {
            title: INCREMENTAL_LENS_TITLE.to_string(),
            command: "dbt.incrementalDiff".to_string(),
            arguments: vec![serde_json::json!({"uri": uri, "show": true})],
        },
    })
}

#[cfg(test)]
#[path = "incremental_diff_tests.rs"]
mod tests;
