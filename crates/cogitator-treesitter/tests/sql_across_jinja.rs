//! SQL string and comment state across Jinja, and byte-accurate spans
//! (specs/rust-port/05 §4 step 2). Mirrors the minijinja pipeline's suite.

use cogitator_dialects::Dialect;
use cogitator_parser::{DocumentParser, ParsedDocument, TokenKind};
use cogitator_treesitter::TreeSitterParser;

fn parse(text: &str) -> ParsedDocument {
    TreeSitterParser::new()
        .expect("language loads")
        .parse(text, &Dialect::new("duckdb"))
        .expect("parse ok")
}

fn word(doc: &ParsedDocument, lit: &str) -> bool {
    doc.tokens
        .iter()
        .any(|t| matches!(t.kind, TokenKind::Word(_)) && doc.literal(t) == lit)
}

fn kinds(doc: &ParsedDocument, kind: TokenKind) -> Vec<&str> {
    doc.tokens
        .iter()
        .filter(|t| t.kind == kind)
        .map(|t| doc.literal(t))
        .collect()
}

#[test]
fn jinja_inside_a_sql_string_keeps_the_surrounding_sql() {
    let doc = parse("with a as (select 1) select * from a where d = '{{ var(\"x\") }}' and e = 1");
    assert!(doc.def_tokens.contains_key("a"));
    assert!(word(&doc, "where") && word(&doc, "e"));
    assert_eq!(kinds(&doc, TokenKind::Var), ["x"]);
}

#[test]
fn jinja_comment_inside_a_sql_string_is_one_string() {
    let doc = parse("select 'a{# c #}b' as d from t");
    assert!(word(&doc, "d") && word(&doc, "t"));
    assert!(!word(&doc, "a") && !word(&doc, "b"));
}

#[test]
fn block_comment_spanning_jinja_is_one_comment() {
    let doc = parse("select a /* {{ x }} */ from t");
    assert!(word(&doc, "a") && word(&doc, "from") && word(&doc, "t"));
    assert!(!doc.tokens.iter().any(|t| t.kind == TokenKind::Symbol('*')));
}

#[test]
fn line_comment_spanning_jinja_is_one_comment() {
    for text in [
        "-- {{ x }} don't\nselect 1 as b",
        "-- {# c #} don't\nselect 1 as b",
        "-- {{ x\n }} don't\nselect 1 as b",
    ] {
        let doc = parse(text);
        assert!(word(&doc, "b"), "{text}");
        assert!(!word(&doc, "don"), "{text}");
        assert!(kinds(&doc, TokenKind::StringLit).is_empty(), "{text}");
    }
}

#[test]
fn comment_text_after_jinja_defines_no_cte() {
    let doc = parse("-- {{ x }} with a bug (see ticket)\nselect 1");
    assert!(doc.def_tokens.is_empty());
}

#[test]
fn ref_in_a_sql_comment_stays_a_dependency() {
    // dbt renders Jinja before SQL, so this ref is real.
    let doc = parse("-- {{ ref('x') }}\nselect 1");
    assert_eq!(kinds(&doc, TokenKind::Ref), ["x"]);
}

#[test]
fn non_ascii_before_a_token_keeps_byte_spans() {
    let doc = parse("select 'äöü' as spalte from t");
    assert!(word(&doc, "as") && word(&doc, "spalte") && word(&doc, "t"));
    assert_eq!(kinds(&doc, TokenKind::StringLit), ["'äöü'"]);

    let doc = parse("/*ééééééééé*/ with x as (select 1) select 1");
    assert!(doc.def_tokens.contains_key("x"));
    assert!(word(&doc, "with"));
}

#[test]
fn token_lengths_come_from_the_source_text() {
    let doc = parse("select 'it''s' as \"quoted id\" from t");
    assert_eq!(kinds(&doc, TokenKind::StringLit), ["'it''s'"]);
    assert!(word(&doc, "\"quoted id\""));
    assert!(word(&doc, "from"));
}

#[test]
fn whitespace_control_keeps_neighbouring_sql() {
    let doc = parse("select a {{- x -}} from t");
    assert!(word(&doc, "a") && word(&doc, "from") && word(&doc, "t"));
}

#[test]
fn quotes_paired_across_branches_are_not_inverted() {
    // Even quote count: lexes "successfully" as one whole-document string
    // running between the branches unless the branch check rejects it.
    let doc = parse("select {% if a %}'x{% else %}'y{% endif %} from t");
    assert!(word(&doc, "select") && word(&doc, "from") && word(&doc, "t"));
    assert!(!word(&doc, "x") && !word(&doc, "y"));

    // Odd quote count: the document is unterminated, so per-segment lexing.
    let doc = parse("select {% if a %}'x'{% else %}'y{% endif %} from t");
    assert_eq!(kinds(&doc, TokenKind::StringLit), ["'x'"]);
    assert!(word(&doc, "from"));
}

#[test]
fn a_construct_around_a_whole_branch_stays_whole() {
    let doc = parse("select '{% if a %}x{% else %}y{% endif %}' as b");
    assert!(word(&doc, "b"));
    assert!(!word(&doc, "x") && !word(&doc, "y"));

    let doc = parse("-- {% if a %} don't\nselect 1 as b\n-- {% endif %} won't\n");
    assert!(word(&doc, "b"));
    assert!(!word(&doc, "don") && !word(&doc, "won"));
}

#[test]
fn unterminated_string_degrades_to_its_own_segment() {
    let doc = parse("select {{ ref('a') }} from t where x = 'oops");
    assert!(word(&doc, "select"));
    assert_eq!(kinds(&doc, TokenKind::Ref), ["a"]);
}
