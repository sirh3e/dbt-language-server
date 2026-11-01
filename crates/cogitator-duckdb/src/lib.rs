//! DuckDB 2.0 parse diagnostics (specs/rust-port/07).
//!
//! The engine integration is feature-gated (`duckdb-parser`) so the bundled
//! C++ build stays out of the dev loop (09 §1). The Jinja masking is always
//! compiled (and tested) — it is pure text work.

use cogitator_parser::{JinjaKind, ParsedDocument, TokenKind};

/// Position-preserving Jinja masking (07 §3.1): every replacement has the
/// original byte length, so DuckDB error offsets map 1:1 onto the template.
/// Returns `None` when the confidence rule (07 §3.2) says not to diagnose:
/// the template contains `{% … %}` control blocks other than config/set.
pub fn mask_jinja(doc: &ParsedDocument) -> Option<String> {
    let text = doc.text.as_bytes();
    let mut out = text.to_vec();

    let mut i = 0;
    while i < doc.tokens.len() {
        let token = &doc.tokens[i];
        if let TokenKind::JinjaOpen(kind) = token.kind {
            // find the matching close
            let close = doc.tokens[i + 1..]
                .iter()
                .position(|t| matches!(t.kind, TokenKind::JinjaClose(k) if k == kind))
                .map(|off| i + 1 + off)?;
            let (start, end) = (token.start as usize, doc.tokens[close].end as usize);
            match kind {
                JinjaKind::Expr => {
                    // `_` + `j` padding: a valid identifier of equal length
                    out[start] = b'_';
                    for b in &mut out[start + 1..end] {
                        *b = b'j';
                    }
                }
                JinjaKind::Stmt => {
                    let first_ident = doc.tokens[i + 1..close]
                        .iter()
                        .find(|t| t.kind == TokenKind::Ident || t.kind == TokenKind::Config);
                    let tag = first_ident
                        .map(|t| &doc.text[t.start as usize..t.end as usize])
                        .unwrap_or("");
                    if !matches!(tag, "config" | "set" | "endset") {
                        return None; // control block → stay silent (07 §3.2)
                    }
                    for b in &mut out[start..end] {
                        if *b != b'\n' {
                            *b = b' ';
                        }
                    }
                }
            }
            i = close;
        }
        i += 1;
    }
    // Comments were swallowed by the tokenizer; blank any `{# … #}` spans.
    blank_comments(&mut out);
    String::from_utf8(out).ok()
}

fn blank_comments(buf: &mut [u8]) {
    let mut i = 0;
    while i + 1 < buf.len() {
        if buf[i] == b'{' && buf[i + 1] == b'#' {
            let mut j = i + 2;
            while j + 1 < buf.len() && !(buf[j] == b'#' && buf[j + 1] == b'}') {
                j += 1;
            }
            let end = (j + 2).min(buf.len());
            for b in &mut buf[i..end] {
                if *b != b'\n' {
                    *b = b' ';
                }
            }
            i = end;
        } else {
            i += 1;
        }
    }
}

#[cfg(feature = "duckdb-parser")]
pub mod engine {
    //! In-process DuckDB 2.0 parser access (07 §2): one lazy in-memory
    //! connection on a blocking worker; `json_serialize_sql` for parse
    //! errors with positions.

    use cogitator_lsp::{diagnostic_severity, Diagnostic, Position, Range};

    pub struct DuckDbParser {
        conn: duckdb::Connection,
    }

    impl DuckDbParser {
        pub fn new() -> Result<Self, String> {
            duckdb::Connection::open_in_memory()
                .map(|conn| DuckDbParser { conn })
                .map_err(|e| e.to_string())
        }

        /// Parse `masked_sql`; a parse error becomes one diagnostic at the
        /// reported offset (07 §3.2).
        pub fn parse_diagnostic(&self, masked_sql: &str, template: &str) -> Option<Diagnostic> {
            let json: String = self
                .conn
                .query_row("SELECT json_serialize_sql(?)", [masked_sql], |row| {
                    row.get(0)
                })
                .ok()?;
            let value: serde_json::Value = serde_json::from_str(&json).ok()?;
            if value.get("error")?.as_bool() != Some(true) {
                return None;
            }
            let message = value
                .get("error_message")
                .and_then(|m| m.as_str())
                .unwrap_or("syntax error")
                .to_string();
            let offset = value
                .get("position")
                .and_then(|p| p.as_str()?.parse::<usize>().ok())
                .or_else(|| {
                    value
                        .get("position")
                        .and_then(|p| p.as_u64())
                        .map(|p| p as usize)
                })
                .unwrap_or(0);
            let (line, character) = line_col(template, offset);
            Some(Diagnostic {
                range: Range::zero_width(Position { line, character }),
                message,
                severity: diagnostic_severity::ERROR,
                code: String::new(),
                source: "duckdb 2.0".to_string(),
            })
        }
    }

    fn line_col(text: &str, offset: usize) -> (u32, u32) {
        let upto = &text.as_bytes()[..offset.min(text.len())];
        let line = upto.iter().filter(|&&b| b == b'\n').count() as u32;
        let last = upto
            .iter()
            .rposition(|&b| b == b'\n')
            .map(|p| p + 1)
            .unwrap_or(0);
        (line, (offset.min(text.len()) - last) as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cogitator_dialects::Dialect;
    use cogitator_parser::{DocumentParser, JinjaSqlParser};

    fn parse(text: &str) -> ParsedDocument {
        JinjaSqlParser.parse(text, &Dialect::new("duckdb")).unwrap()
    }

    #[test]
    fn expr_masked_as_identifier_of_equal_length() {
        let text = "select * from {{ ref('stg_orders') }} where x = 1";
        let masked = mask_jinja(&parse(text)).expect("maskable");
        assert_eq!(masked.len(), text.len());
        assert!(masked.starts_with("select * from _jjjjj"));
        assert!(masked.ends_with("where x = 1"));
        assert!(!masked.contains("{{"));
    }

    #[test]
    fn control_blocks_suppress_masking() {
        let text = "select 1 {% if flag %} ,2 {% endif %}";
        assert_eq!(mask_jinja(&parse(text)), None);
    }

    #[test]
    fn config_blocks_are_blanked_not_suppressed() {
        let text = "{% set x = 1 %}\nselect {{ x }}";
        let masked = mask_jinja(&parse(text)).expect("set block allowed");
        assert_eq!(masked.len(), text.len());
        assert!(masked.starts_with("               \n"));
    }
}
