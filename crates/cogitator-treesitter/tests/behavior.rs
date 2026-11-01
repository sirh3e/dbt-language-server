//! The 02-derived behavior suite run against the tree-sitter backend
//! (06 §3.5: it may become the default only when this passes), plus the
//! error-tolerance property the phase-1 pipeline lacks.

use cogitator_dialects::Dialect;
use cogitator_parser::{DocumentParser, ParsedDocument, TokenKind};
use cogitator_treesitter::TreeSitterParser;
use pretty_assertions::assert_eq;

fn parse(text: &str) -> ParsedDocument {
    TreeSitterParser::new()
        .expect("language loads")
        .parse(text, &Dialect::new("duckdb"))
        .expect("tree-sitter parse never fails on text input")
}

fn kind_of_literal(doc: &ParsedDocument, lit: &str) -> Option<TokenKind> {
    doc.tokens
        .iter()
        .find(|t| doc.literal(t) == lit)
        .map(|t| t.kind)
}

#[test]
fn ref_source_var_refined() {
    let doc = parse(concat!(
        "select * from {{ ref('stg_orders') }}\n",
        "join {{ source(\"jaffle\", 'customers') }} using (id)\n",
        "where amount > {{ var('threshold') }}\n",
    ));
    assert_eq!(kind_of_literal(&doc, "stg_orders"), Some(TokenKind::Ref));
    assert_eq!(kind_of_literal(&doc, "jaffle"), Some(TokenKind::SourceName));
    assert_eq!(
        kind_of_literal(&doc, "customers"),
        Some(TokenKind::SourceTable)
    );
    assert_eq!(kind_of_literal(&doc, "threshold"), Some(TokenKind::Var));
}

#[test]
fn macros_packages_and_ctes() {
    let doc = parse(concat!(
        "with orders as (\n    select * from {{ ref('stg_orders') }}\n)\n",
        "select {{ dbt_utils.star(ref('a')) }}, {{ my_macro(1) }} from orders\n",
    ));
    assert_eq!(kind_of_literal(&doc, "dbt_utils"), Some(TokenKind::Package));
    assert_eq!(kind_of_literal(&doc, "star"), Some(TokenKind::Macro));
    assert_eq!(kind_of_literal(&doc, "my_macro"), Some(TokenKind::Macro));
    assert!(doc.def_tokens.contains_key("orders"));
}

#[test]
fn cursor_and_lookback() {
    let text = "select * from {{ source('jaffle', 'customers') }}";
    let doc = parse(text);
    let col = text.find("customers").unwrap() as u32;
    let idx = doc
        .find_token_at_cursor(0, col + 1)
        .expect("token at cursor");
    assert_eq!(doc.tokens[idx].kind, TokenKind::SourceTable);
    let src = doc
        .lookback_find(idx, TokenKind::SourceName, 4)
        .expect("lookback");
    assert_eq!(doc.literal(src), "jaffle");
}

#[test]
fn comments_are_excluded_from_sql() {
    let doc = parse("select 1 {# ref('not_a_ref') #} from t");
    assert_eq!(kind_of_literal(&doc, "not_a_ref"), None);
}

/// The headline property (06 §3.1): a broken construct degrades locally —
/// the earlier, well-formed ref survives instead of the whole document
/// going stale. The phase-1 pipeline fails this parse outright.
#[test]
fn broken_tail_keeps_earlier_tokens() {
    let text = "select * from {{ ref('stg_orders') }} where x > {{ ref('bro";
    let doc = parse(text);
    assert_eq!(kind_of_literal(&doc, "stg_orders"), Some(TokenKind::Ref));
    assert_eq!(
        kind_of_literal(&doc, "select"),
        Some(TokenKind::Word(cogitator_parser::Keyword::SELECT))
    );

    // contrast: phase-1 errors on the same input
    assert!(cogitator_parser::JinjaSqlParser
        .parse(text, &Dialect::new("duckdb"))
        .is_err());
}

#[test]
fn broken_head_keeps_later_tokens() {
    let text = "{{ ref( \nselect * from {{ ref('stg_orders') }}";
    let doc = parse(text);
    assert_eq!(kind_of_literal(&doc, "stg_orders"), Some(TokenKind::Ref));
}
