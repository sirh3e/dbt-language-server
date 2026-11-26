//! SQL on sqlfluff's Rust lexer and parser (vendor/sqlfluffrs): the
//! dialects, the parse tree the layout is built from, and the check that a
//! layout kept every token.

mod build;
mod doc;
mod events;
mod sep;
mod space;

use std::str::FromStr;
use std::sync::Arc;

use sqlfluffrs_dialects::Dialect;
use sqlfluffrs_lexer::{LexInput, Lexer};
use sqlfluffrs_parser::parser::{intern_indent_config, Node, Parser};
use sqlfluffrs_types::{TemplatedFile, Token};

pub use build::{layout, Placeholders, TagRole};
pub use doc::print;
pub use events::{events, tag_leaf, verbatim_leaf};

/// sqlfluff's dialects, as `dialect =` names them.
pub const DIALECTS: [&str; 28] = [
    "ansi", "athena", "bigquery", "clickhouse", "databricks", "db2", "doris", "duckdb",
    "exasol", "flink", "greenplum", "hive", "impala", "mariadb", "materialize", "mysql",
    "oracle", "postgres", "redshift", "snowflake", "soql", "sparksql", "sqlite", "starrocks",
    "teradata", "trino", "tsql", "vertica",
];

pub fn dialect_names() -> Vec<&'static str> {
    DIALECTS.to_vec()
}

pub fn dialect(name: &str) -> Option<Dialect> {
    Dialect::from_str(name).ok()
}

/// The largest statement the parser takes apart, in nodes; past it a
/// document is declined rather than laid out slowly.
const MAX_PARSE_NODES: usize = 100_000;
const MAX_PARSE_DEPTH: usize = 600;

fn lex(sql: &str, dialect: Dialect) -> Result<Vec<Token>, String> {
    let file = Arc::new(TemplatedFile::new(
        sql.to_string(),
        "<model>".into(),
        None,
        None,
        None,
    ));
    let lexer = Lexer::new(None, dialect.get_lexers().to_vec());
    let (tokens, errors) = lexer.lex(LexInput::TemplatedFile(file), true);
    match errors.first() {
        None => Ok(tokens),
        Some(e) => Err(format!(
            "line {}, column {}: {}",
            e.line_no,
            e.line_pos,
            e.description.as_deref().unwrap_or("cannot be lexed")
        )),
    }
}

/// The parse tree of a SQL text, or where it stops being SQL the parser
/// knows: any unparsable part refuses the whole text.
pub fn parse(sql: &str, dialect: Dialect) -> Result<Node, String> {
    let tokens = lex(sql, dialect)?;
    let mut parser = Parser::new_with_max_parse_depth(
        &tokens,
        dialect,
        intern_indent_config(std::iter::empty::<(String, bool)>()),
        MAX_PARSE_DEPTH,
    )
    .with_node_limit(MAX_PARSE_NODES);
    let tree = parser.root_parse().map_err(|e| format!("{e:?}"))?;
    match unparsable(&tree) {
        None => Ok(tree),
        Some(expected) => Err(format!("unparsable SQL (expected {expected})")),
    }
}

fn unparsable(node: &Node) -> Option<String> {
    match node {
        Node::Unparsable { expected, .. } => Some(expected.clone()),
        // what the grammar matched nothing to comes back as this segment
        Node::Segment {
            segment_type: Some(ty),
            children,
            ..
        } if ty == "unparsable" => {
            let text: String = children.iter().map(Node::raw).collect();
            let line = text.trim().lines().next().unwrap_or("");
            let shown: String = line.chars().take(60).collect();
            let more = if shown.len() < text.trim().len() { " …" } else { "" };
            Some(format!("something else than `{shown}`{more}"))
        }
        Node::Segment { children, .. } => children.iter().find_map(unparsable),
        _ => None,
    }
}

/// The code and comment tokens of a SQL text, in order, as the check
/// compares them: case folded (a case rule may change them), comments
/// trimmed (the layout re-indents them), and symbols that touch as one
/// (`||` is not `| |`, though some dialects lex both as two `|`).
pub fn code_tokens(sql: &str, dialect: Dialect) -> Result<Vec<String>, String> {
    let tokens = lex(sql, dialect)?;
    let mut out: Vec<String> = Vec::new();
    let mut touching = false;
    for token in tokens.iter().filter(|t| !t.is_meta) {
        let raw = token.raw();
        if token.is_whitespace() || token.token_type == "newline" || raw.trim().is_empty() {
            touching = false;
            continue;
        }
        let text = if token.is_comment() {
            raw.trim().lines().map(str::trim).collect::<Vec<_>>().join("\n")
        } else {
            raw.to_uppercase()
        };
        match out.last_mut() {
            Some(last) if touching && space::is_symbol(last) && space::is_symbol(&text) => {
                last.push_str(&text);
            }
            _ => out.push(text),
        }
        touching = true;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_dialect_name_is_one_the_parser_knows() {
        assert!(DIALECTS.iter().all(|name| dialect(name).is_some()));
    }

    #[test]
    fn an_unparsable_part_refuses_the_text() {
        let duckdb = dialect("duckdb").unwrap();
        assert!(parse("select a from t", duckdb).is_ok());
        assert!(parse("select a from t where", duckdb).is_err());
    }

    #[test]
    fn tokens_ignore_layout_and_case() {
        let ansi = dialect("ansi").unwrap();
        let a = code_tokens("select a,b\nfrom t -- note\n", ansi).unwrap();
        let b = code_tokens("SELECT\n    a,\n    b\nFROM t  -- note", ansi).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn touching_symbols_are_one_token_to_the_check() {
        let duckdb = dialect("duckdb").unwrap();
        let glued = code_tokens("select a || b", duckdb).unwrap();
        let apart = code_tokens("select a | | b", duckdb).unwrap();
        assert_ne!(glued, apart);
    }
}
