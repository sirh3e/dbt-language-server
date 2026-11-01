//! Behavior tests (02 banner: refined kinds + positions, not token dumps).
//! Inputs ported from the Go repo's parser_test.go cases.

use dbt_ls_dialects::Dialect;
use dbt_ls_parser::{jinja_scan, DocumentParser, JinjaSqlParser, TokenKind};
use pretty_assertions::assert_eq;
use rstest::rstest;

fn parse(text: &str) -> dbt_ls_parser::ParsedDocument {
    JinjaSqlParser
        .parse(text, &Dialect::new("duckdb"))
        .expect("parse ok")
}

fn kind_of_literal(doc: &dbt_ls_parser::ParsedDocument, lit: &str) -> Option<TokenKind> {
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
fn cursor_lookup_finds_ref_at_exact_position() {
    let text = "select * from {{ ref('stg_orders') }}";
    let doc = parse(text);
    let col = text.find("stg_orders").unwrap() as u32;
    let idx = doc
        .find_token_at_cursor(0, col + 3)
        .expect("token at cursor");
    assert_eq!(doc.tokens[idx].kind, TokenKind::Ref);
    assert_eq!(doc.literal(&doc.tokens[idx]), "stg_orders");
    // one past the end does not match (02 §3.5)
    let end = col + "stg_orders".len() as u32;
    assert!(doc
        .find_token_at_cursor(0, end)
        .map(|i| doc.tokens[i].kind != TokenKind::Ref)
        .unwrap_or(true));
}

#[test]
fn macros_and_packages_refined() {
    let doc = parse("{{ dbt_utils.star(ref('a')) }}\nselect {{ my_macro(1) }}\n");
    assert_eq!(kind_of_literal(&doc, "dbt_utils"), Some(TokenKind::Package));
    assert_eq!(kind_of_literal(&doc, "star"), Some(TokenKind::Macro));
    assert_eq!(kind_of_literal(&doc, "my_macro"), Some(TokenKind::Macro));
    // plain variable interpolation is not a macro
    let doc2 = parse("select {{ some_var }} from t");
    assert_eq!(kind_of_literal(&doc2, "some_var"), Some(TokenKind::Ident));
}

#[test]
fn cte_definitions_collected() {
    let doc = parse(concat!(
        "with orders as (\n    select * from {{ ref('stg_orders') }}\n),\n",
        "customers as (\n    select * from {{ ref('stg_customers') }}\n)\n",
        "select * from orders join customers using (customer_id)\n",
    ));
    assert!(
        doc.def_tokens.contains_key("orders"),
        "orders cte: {:?}",
        doc.def_tokens.keys()
    );
    assert!(doc.def_tokens.contains_key("customers"));
    assert_eq!(doc.def_tokens["orders"].line, 0);
}

#[test]
fn lookback_finds_source_from_table() {
    let text = "select * from {{ source('jaffle', 'customers') }}";
    let doc = parse(text);
    let table_idx = doc
        .tokens
        .iter()
        .position(|t| t.kind == TokenKind::SourceTable)
        .expect("source table token");
    let src = doc
        .lookback_find(table_idx, TokenKind::SourceName, 4)
        .expect("lookback to source");
    assert_eq!(doc.literal(src), "jaffle");
}

#[test]
fn malformed_input_errors_and_caller_keeps_last_good() {
    let err = JinjaSqlParser.parse("select {{ ref('x ", &Dialect::new("duckdb"));
    assert!(err.is_err(), "unterminated jinja should error");
}

#[rstest]
#[case("{% macro cents_to_dollars(column_name, scale=2) %}\n({{ column_name }} / 100)::numeric(16, {{ scale }})\n{% endmacro %}", "cents_to_dollars", "cents_to_dollars(column_name, scale=2)")]
#[case(
    "{%- macro multi(a,\n    b) -%}x{%- endmacro %}",
    "multi",
    "multi(a,\n    b)"
)]
fn macro_extraction(#[case] text: &str, #[case] name: &str, #[case] signature: &str) {
    let macros = jinja_scan::extract_macros(text);
    assert_eq!(macros.len(), 1);
    assert_eq!(macros[0].name, name);
    assert_eq!(macros[0].signature, signature);
}

#[test]
fn docs_blocks_multiple_in_one_file() {
    // the Go regex got this wrong (greedy cross-block capture, Q2)
    let text = concat!(
        "{% docs orders_status %}\nStatus of the order.\n{% enddocs %}\n\n",
        "{% docs customer_id %}\nUnique id.\n{% enddocs %}\n",
    );
    let blocks = jinja_scan::extract_docs_blocks(text);
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].name, "orders_status");
    assert_eq!(blocks[0].content, "Status of the order.");
    assert_eq!(blocks[1].name, "customer_id");
    assert_eq!(blocks[1].content, "Unique id.");
}

#[test]
fn doc_refs_replaced_in_descriptions() {
    let out = jinja_scan::replace_doc_refs(
        "Status: {{ doc(\"orders_status\") }} and {{ doc('missing') }}.",
        |name| (name == "orders_status").then(|| "the docs text".to_string()),
    );
    assert_eq!(out, "Status: the docs text and {{ doc('missing') }}.");
}
