//! What a SQL file depends on, from its own `ref()` and `source()`
//! tokens: the project graph keeps source edges by source name alone, and
//! scans no snapshot, so a selector reads the files itself.

use cogitator_dialects::Dialect;
use cogitator_parser::{DocumentParser, JinjaSqlParser, TokenKind};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct SqlDeps {
    pub refs: Vec<String>,
    pub sources: Vec<(String, String)>,
}

/// Every `ref(...)` and `source(name, table)` a SQL file calls, each once.
pub fn sql_deps(text: &str, dialect: &Dialect) -> SqlDeps {
    let Ok(doc) = JinjaSqlParser.parse(text, dialect) else {
        return SqlDeps::default();
    };
    let mut out = SqlDeps::default();
    let mut pending: Option<String> = None;
    for token in &doc.tokens {
        match token.kind {
            TokenKind::Ref => {
                let name = doc.literal(token).to_string();
                if !out.refs.contains(&name) {
                    out.refs.push(name);
                }
            }
            TokenKind::SourceName => pending = Some(doc.literal(token).to_string()),
            TokenKind::SourceTable => {
                if let Some(source) = pending.take() {
                    let pair = (source, doc.literal(token).to_string());
                    if !out.sources.contains(&pair) {
                        out.sources.push(pair);
                    }
                }
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refs_and_source_pairs_are_read_from_tokens() {
        let sql = "select * from {{ ref('a') }} join {{ source('raw', 'orders') }} \
                   join {{ source(\"raw\", \"payments\") }} join {{ ref('a') }} \
                   join {{ source('raw', 'orders') }}";
        assert_eq!(
            sql_deps(sql, &Dialect::default()),
            SqlDeps {
                refs: vec!["a".into()],
                sources: vec![
                    ("raw".into(), "orders".into()),
                    ("raw".into(), "payments".into())
                ],
            }
        );
    }
}
