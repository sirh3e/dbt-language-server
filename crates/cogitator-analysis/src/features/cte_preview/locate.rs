//! Where the preview lenses sit: one "Preview rows" on every CTE of the
//! model's top-level WITH clause and one on its final select, read from
//! the document's tokens (the parser's CTE definitions, paren depth from
//! its symbols) — and which CTE a cursor is in.

use cogitator_lsp::{CodeLens, Command, Position, Range};
use cogitator_parser::{Keyword, ParsedDocument, Token, TokenKind};
use serde_json::json;

use super::super::{model_name_from_uri, DbtContext, Document};

/// The command the lenses run.
pub const PREVIEW_CTE: &str = "dbt.previewCte";

/// The lenses of a model file the context knows; none elsewhere.
pub fn cte_preview_lenses(ctx: &DbtContext, doc: &Document, uri: &str) -> Vec<CodeLens> {
    match model_name_from_uri(uri) {
        Some(model) if ctx.model_detail_map.contains_key(&model) => lenses(&doc.parsed, uri),
        _ => Vec::new(),
    }
}

fn lenses(parsed: &ParsedDocument, uri: &str) -> Vec<CodeLens> {
    let outline = outline(parsed);
    let mut lenses: Vec<CodeLens> = outline
        .ctes
        .iter()
        .map(|(name, token)| lens(token, json!({"uri": uri, "cte": name})))
        .collect();
    if let Some(select) = outline.final_select {
        lenses.push(lens(&select, json!({"uri": uri})));
    }
    lenses
}

/// The CTE the cursor is in: the last top-level CTE starting at or
/// before it, while the cursor is before the final select.
pub fn cte_at(parsed: &ParsedDocument, position: Position) -> Option<String> {
    let outline = outline(parsed);
    let at = (position.line, position.character);
    if outline
        .final_select
        .is_some_and(|s| (s.line, s.column) <= at)
    {
        return None;
    }
    outline
        .ctes
        .iter()
        .rev()
        .find(|(_, t)| (t.line, t.column) <= at)
        .map(|(name, _)| name.clone())
}

struct Outline {
    /// Top-level CTEs in document order.
    ctes: Vec<(String, Token)>,
    final_select: Option<Token>,
}

fn outline(parsed: &ParsedDocument) -> Outline {
    let mut depth = 0i32;
    let mut ctes = Vec::new();
    let mut final_select = None;
    let mut last_cte_start = None;
    for token in &parsed.tokens {
        match token.kind {
            TokenKind::Symbol('(') => depth += 1,
            TokenKind::Symbol(')') => depth -= 1,
            _ if depth == 0 => {
                let name = parsed.literal(token);
                let is_def = parsed
                    .def_tokens
                    .get(name)
                    .is_some_and(|def| def.start == token.start);
                if is_def {
                    ctes.push((name.to_string(), *token));
                    last_cte_start = Some(token.start);
                    final_select = None;
                } else if token.kind == TokenKind::Word(Keyword::SELECT)
                    && final_select.is_none()
                    && last_cte_start.is_none_or(|s| token.start > s)
                {
                    final_select = Some(*token);
                }
            }
            _ => {}
        }
    }
    Outline { ctes, final_select }
}

fn lens(token: &Token, argument: serde_json::Value) -> CodeLens {
    let start = Position {
        line: token.line,
        character: token.column,
    };
    CodeLens {
        range: Range {
            start,
            end: Position {
                line: token.line,
                character: token.column + token.len(),
            },
        },
        command: Command {
            title: "Preview rows".to_string(),
            command: PREVIEW_CTE.to_string(),
            arguments: vec![argument],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cogitator_dialects::Dialect;
    use cogitator_parser::{DocumentParser, JinjaSqlParser};

    const SQL: &str = "with orders as (\n    select * from {{ ref('stg_orders') }}\n),\n\
                       payments as (\n    select id from (select 1 as id) as sub\n)\n\
                       select * from orders\njoin payments using (id)\n";

    fn parsed(sql: &str) -> ParsedDocument {
        JinjaSqlParser.parse(sql, &Dialect::new("duckdb")).unwrap()
    }

    #[test]
    fn a_lens_per_cte_and_one_on_the_final_select() {
        let got: Vec<(u32, serde_json::Value)> = lenses(&parsed(SQL), "file:///m.sql")
            .into_iter()
            .map(|l| (l.range.start.line, l.command.arguments[0].clone()))
            .collect();
        assert_eq!(
            got,
            vec![
                (0, json!({"uri": "file:///m.sql", "cte": "orders"})),
                (3, json!({"uri": "file:///m.sql", "cte": "payments"})),
                (6, json!({"uri": "file:///m.sql"})),
            ]
        );
    }

    #[test]
    fn a_model_without_ctes_gets_the_final_select_lens_only() {
        let got = lenses(&parsed("select 1 as id\n"), "file:///m.sql");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].command.arguments[0], json!({"uri": "file:///m.sql"}));
    }

    #[test]
    fn the_cursor_names_its_cte() {
        let doc = parsed(SQL);
        let cases = [
            ((0, 0), None),
            ((0, 6), Some("orders")),
            ((1, 10), Some("orders")),
            ((4, 30), Some("payments")),
            ((6, 3), None),
            ((7, 2), None),
        ];
        for ((line, character), want) in cases {
            assert_eq!(
                cte_at(&doc, Position { line, character }).as_deref(),
                want,
                "{line}:{character}"
            );
        }
    }
}
