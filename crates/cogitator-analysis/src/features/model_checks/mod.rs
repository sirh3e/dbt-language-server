//! Invalid models as you type: what makes dbt refuse a model, or build
//! something other than what it says, found in the model's own SQL and
//! the project graph, with no `dbt parse` first.
//!
//! - a materialization neither dbt nor the project defines
//!   ([`MATERIALIZATION_CODE`]), a config key that is a near miss of one
//!   dbt reads ([`CONFIG_KEY_CODE`]) and a config value dbt does not take
//!   ([`CONFIG_VALUE_CODE`]);
//! - a model that refs itself ([`SELF_REF_CODE`]) or closes a cycle
//!   through other models ([`REF_CYCLE_CODE`]);
//! - `is_incremental()` in a model that says it is not incremental
//!   ([`NEVER_INCREMENTAL_CODE`]), and a model with no SQL at all
//!   ([`EMPTY_MODEL_CODE`]).
//!
//! The confidence rule of the syntax checks holds here too: what cannot
//! be known from the text (an adapter's own keys and materializations, a
//! value computed by a macro) is never reported. Every finding that has
//! a fix carries it as typed data, so the editor offers it as a
//! quickfix. Pure over the context and the document.

mod config;
mod refs;

#[cfg(test)]
mod tests;

use cogitator_error::guidance::Guidance;
use cogitator_lsp::{
    diagnostic_severity, Diagnostic, DiagnosticData, FixKind, MultiEditFix, Position, Range,
    TextEdit,
};
use cogitator_parser::LineTable;

use super::{model_name_from_uri, DbtContext, Document};
use crate::context::ModelKind;

/// A materialization that neither dbt nor the project defines.
pub const MATERIALIZATION_CODE: &str = "invalid-materialization";
/// A config key that is a near miss of a key dbt reads.
pub const CONFIG_KEY_CODE: &str = "unknown-config-key";
/// A config value outside the values dbt takes for its key.
pub const CONFIG_VALUE_CODE: &str = "invalid-config-value";
/// A model that refs itself.
pub const SELF_REF_CODE: &str = "ref-self";
/// A ref that closes a cycle through other models.
pub const REF_CYCLE_CODE: &str = "ref-cycle";
/// `is_incremental()` in a model that is not incremental.
pub const NEVER_INCREMENTAL_CODE: &str = "is-incremental-never";
/// A model with no SQL.
pub const EMPTY_MODEL_CODE: &str = "empty-model";

/// Every code of this module, for the clients that group by code.
pub const CODES: &[&str] = &[
    MATERIALIZATION_CODE,
    CONFIG_KEY_CODE,
    CONFIG_VALUE_CODE,
    SELF_REF_CODE,
    REF_CYCLE_CODE,
    NEVER_INCREMENTAL_CODE,
    EMPTY_MODEL_CODE,
];

/// One problem: a byte span of the model's text, how bad it is, what to
/// say and what would fix it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Finding {
    pub start: usize,
    pub end: usize,
    pub code: &'static str,
    pub severity: i32,
    pub guidance: GuidanceText,
    pub fix: Fix,
}

/// The words of a finding (kept apart from `Guidance`, which is not
/// comparable, so the findings can be tested as values).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct GuidanceText {
    pub headline: String,
    pub reason: Option<String>,
    pub solution: Option<String>,
}

/// What would fix a finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Fix {
    None,
    /// Replace the finding's span with one of these, closest first.
    Replace(Vec<String>),
    /// Replace another span of the text with a text, under a title.
    Rewrite {
        title: String,
        start: usize,
        end: usize,
        text: String,
    },
}

/// The invalid-model diagnostics of one document. Only a model of the
/// project is checked (a seed, a snapshot, a macro or an analysis is
/// not), and nothing while the project's models are not known yet.
pub fn model_diagnostics(ctx: &DbtContext, doc: &Document, uri: &str) -> Vec<Diagnostic> {
    let Some(model) = model_of(ctx, uri) else {
        return Vec::new();
    };
    let text: &str = &doc.text;
    let table = LineTable::new(text);
    findings(ctx, doc, &model)
        .into_iter()
        .map(|found| diagnostic(text, &table, found))
        .collect()
}

/// Every finding of a model's text, in the order of the text.
pub(crate) fn findings(ctx: &DbtContext, doc: &Document, model: &str) -> Vec<Finding> {
    let text: &str = &doc.text;
    let calls = config::calls(text);
    let mut out = config::check(ctx, &calls);
    out.extend(config::never_incremental(text, &calls));
    out.extend(config::empty_model(text));
    out.extend(refs::check(ctx, doc, model));
    out.sort_by_key(|f| (f.start, f.end));
    out
}

/// The model `uri` defines: one the project knows as a model in that
/// file, or a new file under a model path that the project has not
/// scanned yet (a model being written).
fn model_of(ctx: &DbtContext, uri: &str) -> Option<String> {
    if !super::syntax::applies_to(uri) || ctx.model_detail_map.is_empty() {
        return None;
    }
    let name = model_name_from_uri(uri)?;
    let path = cogitator_lsp::uri_to_fs_path(uri).unwrap_or_else(|| uri.to_string());
    let path = std::path::Path::new(&path);
    match ctx.model_detail_map.get(&name) {
        Some(details) => {
            let same_file = details.uri.as_os_str().is_empty() || details.uri == path;
            (details.kind == ModelKind::Model && same_file).then_some(name)
        }
        None => {
            let root = ctx.project_root.as_deref()?;
            let under_models = ctx
                .project
                .model_paths
                .iter()
                .any(|dir| path.starts_with(root.join(dir)));
            under_models.then_some(name)
        }
    }
}

fn diagnostic(text: &str, table: &LineTable, found: Finding) -> Diagnostic {
    let range = range_of(text, table, found.start, found.end);
    let suggestions = match &found.fix {
        Fix::Replace(s) => s.clone(),
        _ => Vec::new(),
    };
    let mut guidance = Guidance::new(found.guidance.headline).suggest(suggestions.clone());
    if let Some(reason) = found.guidance.reason {
        guidance = guidance.because(reason);
    }
    if let Some(solution) = found.guidance.solution {
        guidance = guidance.fix(solution);
    }
    let mut message = guidance.to_diagnostic();
    if let Some(reason) = &guidance.reason {
        message.push_str(if message.ends_with('?') { " " } else { ". " });
        message.push_str(reason.trim_end_matches('.'));
        message.push('.');
    }
    let mut diagnostic = Diagnostic::plain(range, message, found.severity, found.code, "cogitator");
    diagnostic.data = match found.fix {
        Fix::None => None,
        Fix::Replace(suggestions) => Some(DiagnosticData {
            fix: FixKind::ReplaceToken,
            token_range: range,
            suggestions,
        }),
        Fix::Rewrite {
            title,
            start,
            end,
            text: new_text,
        } => Some(DiagnosticData {
            fix: FixKind::MultiEdit {
                fixes: vec![MultiEditFix {
                    title,
                    edits: vec![TextEdit {
                        range: range_of(text, table, start, end),
                        new_text,
                    }],
                    preserving: false,
                }],
            },
            token_range: range,
            suggestions: Vec::new(),
        }),
    };
    diagnostic
}

fn range_of(text: &str, table: &LineTable, start: usize, end: usize) -> Range {
    let position = |offset: usize| {
        let (line, character) = table.line_col(offset.min(text.len()) as u32);
        Position { line, character }
    };
    Range {
        start: position(start),
        end: position(end.max(start)),
    }
}

/// A finding with no fix.
pub(crate) fn finding(
    (start, end): (usize, usize),
    code: &'static str,
    severity: i32,
    headline: String,
) -> Finding {
    Finding {
        start,
        end,
        code,
        severity,
        guidance: GuidanceText {
            headline,
            ..GuidanceText::default()
        },
        fix: Fix::None,
    }
}

impl Finding {
    pub(crate) fn because(mut self, reason: impl Into<String>) -> Self {
        self.guidance.reason = Some(reason.into());
        self
    }

    pub(crate) fn solved_by(mut self, solution: impl Into<String>) -> Self {
        self.guidance.solution = Some(solution.into());
        self
    }

    pub(crate) fn fixed_by(mut self, fix: Fix) -> Self {
        self.fix = fix;
        self
    }
}

pub(crate) const ERROR: i32 = diagnostic_severity::ERROR;
pub(crate) const WARNING: i32 = diagnostic_severity::WARNING;
