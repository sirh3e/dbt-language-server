//! `column-type-mismatch`: a YAML `data_type:` the model's SQL provably
//! cannot produce, and the two fixes — set one column's type, or fill
//! every missing type of a contracted model from what was inferred.
//!
//! Only `Proven` types speak (see `basis`), only a different family of
//! types disagrees (see `compare`), and a column the model does not
//! produce is left to `contract-drift` / `yaml-unproduced-column`, so a
//! problem is never reported twice. Under `contract: {enforced: true}`
//! dbt build fails on the mismatch, so it is an Error there.

use cogitator_error::guidance::Guidance;
use cogitator_lsp::{
    diagnostic_severity, CodeAction, Diagnostic, DiagnosticData, DocumentChange, FixKind,
    MultiEditFix, Position, Range, TextDocumentEdit, TextEdit, WorkspaceEdit,
};
use cogitator_sql_front::parse_sql_type;

use super::basis::ModelColumnTypes;
use super::compare::{compare, describe, Verdict};
use super::yaml_columns::{written_models, WrittenColumn};
use crate::DbtContext;

pub const MISMATCH: &str = "column-type-mismatch";

/// The mismatches of every model entry written in `yaml_text`. `types`
/// looks a model's inferred columns up (the shell reads the snapshot).
pub fn type_mismatch_diagnostics(
    ctx: &DbtContext,
    yaml_text: &str,
    types: &dyn Fn(&str) -> Option<ModelColumnTypes>,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for model in written_models(yaml_text) {
        let Some(details) = ctx.model_detail_map.get(&model.name) else {
            continue;
        };
        let Some(inferred) = types(&model.name) else {
            continue;
        };
        for column in &model.columns {
            out.extend(mismatch(ctx, column, &inferred, details.contract_enforced));
        }
    }
    out
}

fn mismatch(
    ctx: &DbtContext,
    column: &WrittenColumn,
    inferred: &ModelColumnTypes,
    contract: bool,
) -> Option<Diagnostic> {
    let (raw, range) = column.data_type.as_ref()?;
    if raw.contains("{{") || raw.contains("{%") {
        return None; // rendered by Jinja: not the text dbt compares
    }
    let proven = inferred.column(&column.name).filter(|c| c.is_proven())?;
    let declared = parse_sql_type(raw, &ctx.dialect);
    if compare(&proven.sql_type, &declared) != Verdict::Disagree {
        return None;
    }
    let consequence = if contract {
        "dbt build fails this enforced contract"
    } else {
        "the declared type is wrong"
    };
    let guidance = Guidance::new(format!(
        "'{}.{}' is declared '{raw}' but its SQL yields {} ({}) — {consequence}",
        column.model, column.name, proven.description, proven.why
    ))
    .fix("set data_type to the produced type, or cast the column in the model.");
    let severity = if contract {
        diagnostic_severity::ERROR
    } else {
        diagnostic_severity::WARNING
    };
    let mut diagnostic = Diagnostic::plain(
        *range,
        guidance.to_diagnostic(),
        severity,
        MISMATCH,
        "cogitator",
    );
    if let Some(spelled) = &proven.data_type {
        diagnostic.data = Some(DiagnosticData {
            fix: FixKind::MultiEdit {
                fixes: vec![MultiEditFix {
                    title: format!("Set data_type to {spelled}"),
                    edits: vec![TextEdit {
                        range: *range,
                        new_text: spelled.clone(),
                    }],
                    preserving: false,
                }],
            },
            token_range: *range,
            suggestions: vec![spelled.clone()],
        });
    }
    Some(diagnostic)
}

/// "Fill data_type from the inferred types": in the entry of a model with
/// an enforced contract under the cursor, a `data_type:` line for every
/// column that has none and whose type is proven and spelled exactly.
pub fn fill_data_types_action(
    ctx: &DbtContext,
    yaml_text: &str,
    uri: &str,
    range: Range,
    types: &dyn Fn(&str) -> Option<ModelColumnTypes>,
) -> Option<CodeAction> {
    let line = range.start.line;
    let model = written_models(yaml_text)
        .into_iter()
        .find(|m| m.first_line <= line && line <= m.last_line)?;
    if !ctx
        .model_detail_map
        .get(&model.name)
        .is_some_and(|d| d.contract_enforced)
    {
        return None;
    }
    let inferred = types(&model.name)?;
    let edits: Vec<TextEdit> = model
        .columns
        .iter()
        .filter(|c| c.data_type.is_none() && !c.flow)
        .filter_map(|c| {
            let typed = inferred.column(&c.name).filter(|t| t.is_proven())?;
            let spelled = typed.data_type.as_ref()?;
            Some(TextEdit {
                range: Range::zero_width(Position {
                    line: c.name_range.start.line + 1,
                    character: 0,
                }),
                new_text: format!(
                    "{}data_type: {spelled}\n",
                    " ".repeat(c.key_start.character as usize)
                ),
            })
        })
        .collect();
    if edits.is_empty() {
        return None;
    }
    Some(CodeAction {
        title: format!(
            "Fill data_type from the inferred types ({} column(s) of '{}')",
            edits.len(),
            model.name
        ),
        kind: Some("quickfix"),
        diagnostics: None,
        edit: Some(WorkspaceEdit {
            document_changes: vec![DocumentChange::Edit(TextDocumentEdit::new(
                uri.to_string(),
                edits,
            ))],
        }),
        command: None,
        is_preferred: None,
        data: None,
    })
}

/// The hover line of a column written in a properties file at `pos`.
pub fn yaml_column_hover(
    ctx: &DbtContext,
    yaml_text: &str,
    pos: Position,
    types: &dyn Fn(&str) -> Option<ModelColumnTypes>,
) -> Option<String> {
    let models = written_models(yaml_text);
    let column = models
        .iter()
        .flat_map(|m| &m.columns)
        .find(|c| contains(c.name_range, pos))?;
    let typed = types(&column.model)?;
    let inferred = typed.column(&column.name)?;
    let mut text = super::hover::type_line(inferred);
    if let Some((raw, _)) = &column.data_type {
        let declared = parse_sql_type(raw, &ctx.dialect);
        if inferred.is_proven() && compare(&inferred.sql_type, &declared) == Verdict::Disagree {
            text.push_str(&format!(
                "\n\nDeclared `{raw}` ({}), which the SQL does not produce.",
                describe(&declared, &ctx.dialect)
            ));
        }
    }
    Some(text)
}

fn contains(range: Range, pos: Position) -> bool {
    range.start.line == pos.line
        && range.start.character <= pos.character
        && pos.character <= range.end.character
}
