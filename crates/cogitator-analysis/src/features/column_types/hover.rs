//! Hover lines for typed columns: the type and, briefly, why.

use cogitator_lsp::Position;
use cogitator_parser::TokenKind;

use super::basis::{Basis, ColumnType, ModelColumnTypes};
use crate::Document;

/// `Type: \`bigint\` — an explicit cast`, with the evidence when it is
/// weaker than proof.
pub fn type_line(column: &ColumnType) -> String {
    let mut text = format!("Type: `{}` — {}", column.description, column.why);
    match column.basis {
        Basis::Proven => {}
        Basis::Declared => text.push_str(" (rests on a YAML data_type nothing enforces)"),
        Basis::Unknown => return format!("Type: unknown — {}", column.why),
    }
    match column.nullable {
        Some(false) => text.push_str("; never null"),
        Some(true) => text.push_str("; nullable"),
        None => {}
    }
    text
}

/// On a name in a model's SQL that is one of its output columns: that
/// column's type line. The types come from the saved file's inference.
pub fn sql_column_hover(doc: &Document, pos: Position, types: &ModelColumnTypes) -> Option<String> {
    let parsed = &doc.parsed;
    let idx = parsed.find_token_at_cursor(pos.line, pos.character)?;
    let token = &parsed.tokens[idx];
    if !matches!(token.kind, TokenKind::Word(_)) {
        return None;
    }
    let word = parsed.literal(token);
    let word = word.trim_matches(|c| c == '"' || c == '`');
    let column = types.column(word)?;
    Some(format!(
        "Output column `{}` of {}\n\n{}",
        column.name,
        types.model,
        type_line(column)
    ))
}
