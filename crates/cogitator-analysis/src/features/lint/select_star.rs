//! `SELECT *` outside staging: downstream contracts break silently when
//! an upstream adds columns. When the document reads from exactly one
//! documented model, the explicit column list rides as a quickfix.

use cogitator_error::guidance::Guidance;
use cogitator_lsp::{diagnostic_severity, Diagnostic, DiagnosticData, FixKind};
use cogitator_parser::{Keyword, TokenKind};

use super::super::completion::scope::{RelationTarget, ScopeIndex};
use super::super::{token_range, DbtContext, Document};

pub fn check(ctx: &DbtContext, doc: &Document, uri: &str) -> Vec<Diagnostic> {
    if super::is_staging(uri) {
        return Vec::new();
    }
    let tokens = &doc.parsed.tokens;
    let mut out = Vec::new();
    for window in tokens.windows(2) {
        let [select, star] = window else { continue };
        if select.kind != TokenKind::Word(Keyword::SELECT) || star.kind != TokenKind::Symbol('*') {
            continue;
        }
        let range = token_range(star);
        let guidance = Guidance::new("SELECT * in a non-staging model".to_string())
            .because("downstream contracts break silently when the upstream adds columns.")
            .fix("list the columns explicitly.");
        let mut diagnostic = Diagnostic::plain(
            range,
            guidance.to_diagnostic(),
            diagnostic_severity::WARNING,
            "lint-select-star",
            "cogitator",
        );
        if let Some(columns) = single_relation_columns(ctx, doc) {
            diagnostic.data = Some(DiagnosticData {
                fix: FixKind::ReplaceToken,
                token_range: range,
                suggestions: vec![columns],
            });
        }
        out.push(diagnostic);
    }
    out
}

/// The explicit column list, when the document reads from exactly one
/// relation whose columns the YAML documents.
fn single_relation_columns(ctx: &DbtContext, doc: &Document) -> Option<String> {
    let scope = ScopeIndex::build(doc);
    let relations = scope.relations();
    let [(_, target)] = relations else {
        return None;
    };
    let columns: Vec<&str> = match target {
        RelationTarget::Model(name) => ctx
            .model_detail_map
            .get(name)?
            .columns
            .iter()
            .map(|c| c.name.as_str())
            .collect(),
        RelationTarget::SourceTable { source, table } => ctx
            .source_detail_map
            .get(source)?
            .tables
            .get(table)?
            .columns
            .iter()
            .map(|c| c.name.as_str())
            .collect(),
        _ => return None,
    };
    (!columns.is_empty()).then(|| columns.join(", "))
}
