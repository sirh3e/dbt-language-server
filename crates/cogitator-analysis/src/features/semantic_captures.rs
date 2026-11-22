//! Semantic tokens from highlight captures (specs/highlighting §12): the
//! spans `dbt.highlight` answers for a YAML property file — the dbt-aware
//! captures of §5 and the Jinja and SQL inside values — as the standard
//! `textDocument/semanticTokens` stream, so a client that ignores the
//! custom channel still colours them. One source of truth: the server feeds
//! the very spans `dbt.highlight` returns; this module only maps names to
//! the legend and encodes.

use cogitator_lsp::{SemanticTokens, SEMANTIC_TOKEN_MODIFIERS, SEMANTIC_TOKEN_TYPES};

/// One single-line highlight span, UTF-16 columns, with the capture and
/// the layer (`yaml`, `sql`, `jinja`, `dbt`) that produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureSpan<'a> {
    pub line: u32,
    pub character: u32,
    pub length: u32,
    pub capture: &'a str,
    pub layer: &'a str,
}

/// Leading capture segments → a legend type, first match wins.
const TYPE_OF_PREFIX: &[(&str, &str)] = &[
    ("function.dbt.macro", "macro"),
    ("module.dbt.source", "namespace"),
    ("variable.parameter", "parameter"),
    ("function.macro", "macro"),
    ("keyword", "keyword"),
    ("function", "function"),
    ("method", "function"),
    ("variable", "variable"),
    ("string", "string"),
    ("number", "number"),
    ("float", "number"),
    ("boolean", "keyword"),
    ("operator", "operator"),
    ("type", "type"),
    ("property", "property"),
    ("field", "property"),
    ("module", "namespace"),
    ("namespace", "namespace"),
];

/// dbt-aware captures naming what a YAML entry defines (§5 table).
const DECLARATIONS: &[&str] = &[
    "type.dbt.model",
    "type.dbt.snapshot",
    "type.dbt.seed",
    "type.dbt.analysis",
    "type.dbt.exposure",
    "type.dbt.metric",
    "type.dbt.semantic_model",
    "type.dbt.saved_query",
    "type.dbt.source_table",
    "module.dbt.source",
    "property.dbt.column",
    "function.dbt.macro",
    "function.dbt.selector",
];

fn index_of(list: &[&str], name: &str) -> u32 {
    list.iter().position(|t| *t == name).expect("in the legend") as u32
}

/// The `(token type, modifier bits)` of a capture, `None` when it has no
/// place in the legend (punctuation, comments, the YAML base layer — the
/// client colours YAML syntax itself).
pub fn capture_token(capture: &str, layer: &str) -> Option<(u32, u32)> {
    if layer == "yaml" {
        return None;
    }
    let unresolved = capture.ends_with(".unresolved");
    let base = capture.strip_suffix(".unresolved").unwrap_or(capture);
    let matches = |prefix: &str| {
        base == prefix
            || base
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.starts_with('.'))
    };
    let ty = TYPE_OF_PREFIX
        .iter()
        .find(|(prefix, _)| matches(prefix))
        .map(|(_, ty)| index_of(SEMANTIC_TOKEN_TYPES, ty))?;
    let bit = |name: &str| 1u32 << index_of(SEMANTIC_TOKEN_MODIFIERS, name);
    let mut modifiers = 0;
    let dbt = layer == "dbt" || base.split('.').any(|s| s == "dbt");
    if dbt {
        modifiers |= bit("dbt");
    }
    if unresolved {
        modifiers |= bit("unresolved");
    }
    if DECLARATIONS.contains(&base) {
        modifiers |= bit("declaration");
    }
    Some((ty, modifiers))
}

/// The LSP delta encoding of the spans that map to the legend, in
/// document order; empty spans and overlaps (a later span starting inside
/// an earlier one) are dropped, as the protocol forbids them.
pub fn encode_captures(spans: &[CaptureSpan]) -> SemanticTokens {
    let mut kept: Vec<(u32, u32, u32, u32, u32)> = spans
        .iter()
        .filter(|s| s.length > 0)
        .filter_map(|s| {
            capture_token(s.capture, s.layer).map(|(ty, m)| (s.line, s.character, s.length, ty, m))
        })
        .collect();
    kept.sort_by_key(|t| (t.0, t.1));
    let mut data = Vec::with_capacity(kept.len() * 5);
    let (mut line, mut col, mut end) = (0u32, 0u32, 0u32);
    let mut first = true;
    for (l, c, len, ty, m) in kept {
        if !first && l == line && c < end {
            continue;
        }
        let delta_line = l - line;
        let delta_col = if first || delta_line > 0 { c } else { c - col };
        data.extend_from_slice(&[delta_line, delta_col, len, ty, m]);
        (line, col, end, first) = (l, c, c + len, false);
    }
    SemanticTokens { data }
}

/// `textDocument/semanticTokens/range`: the tokens of a full stream on
/// lines `start..=end`, re-encoded relative to each other.
pub fn tokens_in_lines(tokens: &SemanticTokens, start: u32, end: u32) -> SemanticTokens {
    let (mut line, mut col) = (0u32, 0u32);
    let mut out = Vec::new();
    let (mut prev_line, mut prev_col, mut first) = (0u32, 0u32, true);
    for chunk in tokens.data.as_chunks::<5>().0 {
        line += chunk[0];
        col = if chunk[0] == 0 {
            col + chunk[1]
        } else {
            chunk[1]
        };
        if line < start || line > end {
            continue;
        }
        let delta_line = if first { line } else { line - prev_line };
        let delta_col = if first || delta_line > 0 {
            col
        } else {
            col - prev_col
        };
        out.extend_from_slice(&[delta_line, delta_col, chunk[2], chunk[3], chunk[4]]);
        (prev_line, prev_col, first) = (line, col, false);
    }
    SemanticTokens { data: out }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn ty(name: &str) -> u32 {
        index_of(SEMANTIC_TOKEN_TYPES, name)
    }
    const DECL: u32 = 1;
    const DBT: u32 = 2;
    const UNRESOLVED: u32 = 4;

    #[rstest]
    #[case("type.dbt.model", "dbt", Some(("type", DBT | DECL)))]
    #[case("type.dbt.reference", "dbt", Some(("type", DBT)))]
    #[case(
        "type.dbt.reference.unresolved",
        "dbt",
        Some(("type", DBT | UNRESOLVED))
    )]
    #[case("module.dbt.source", "dbt", Some(("namespace", DBT | DECL)))]
    #[case("type.dbt.source_table", "dbt", Some(("type", DBT | DECL)))]
    #[case("property.dbt.column", "dbt", Some(("property", DBT | DECL)))]
    #[case("function.dbt.test", "dbt", Some(("function", DBT)))]
    #[case("function.dbt.macro", "dbt", Some(("macro", DBT | DECL)))]
    #[case("variable.parameter.dbt", "dbt", Some(("parameter", DBT)))]
    #[case("keyword.dbt.config", "dbt", Some(("keyword", DBT)))]
    #[case("string.special.dbt.doc", "dbt", Some(("string", DBT)))]
    #[case(
        "string.special.dbt.doc.unresolved",
        "dbt",
        Some(("string", DBT | UNRESOLVED))
    )]
    #[case("type.dbt.profile", "dbt", Some(("type", DBT)))]
    #[case("keyword.conditional", "sql", Some(("keyword", 0)))]
    #[case("function.call", "jinja", Some(("function", 0)))]
    #[case("variable", "jinja", Some(("variable", 0)))]
    #[case("number", "sql", Some(("number", 0)))]
    #[case("punctuation.bracket", "jinja", None)]
    #[case("tag.delimiter", "jinja", None)]
    #[case("comment", "sql", None)]
    #[case("property", "yaml", None)]
    #[case("keywordish", "sql", None)]
    fn captures_map_to_the_legend(
        #[case] capture: &str,
        #[case] layer: &str,
        #[case] expected: Option<(&str, u32)>,
    ) {
        assert_eq!(
            capture_token(capture, layer),
            expected.map(|(t, m)| (ty(t), m))
        );
    }

    fn span<'a>(line: u32, character: u32, length: u32, capture: &'a str) -> CaptureSpan<'a> {
        CaptureSpan {
            line,
            character,
            length,
            capture,
            layer: "dbt",
        }
    }

    #[test]
    fn spans_encode_as_deltas_in_document_order() {
        let tokens = encode_captures(&[
            span(3, 4, 6, "property.dbt.column"),
            span(1, 10, 9, "type.dbt.model"),
            span(3, 12, 2, "keyword.dbt.config"),
            span(3, 0, 0, "type.dbt.model"),
            CaptureSpan {
                layer: "yaml",
                ..span(2, 0, 4, "property")
            },
        ]);
        assert_eq!(
            tokens.data,
            vec![
                1,
                10,
                9,
                ty("type"),
                DBT | DECL,
                2,
                4,
                6,
                ty("property"),
                DBT | DECL,
                0,
                8,
                2,
                ty("keyword"),
                DBT
            ]
        );
    }

    #[test]
    fn overlapping_spans_keep_the_first() {
        let tokens = encode_captures(&[
            span(0, 0, 5, "type.dbt.model"),
            span(0, 2, 2, "keyword.dbt.config"),
            span(0, 5, 1, "keyword.dbt.config"),
        ]);
        assert_eq!(tokens.data.len(), 10);
        assert_eq!(&tokens.data[5..7], &[0, 5]);
    }

    #[rstest]
    #[case(0, 9, vec![1, 10, 9, 0, 0, 2, 4, 6, 0, 0, 0, 8, 2, 0, 0])]
    #[case(3, 3, vec![3, 4, 6, 0, 0, 0, 8, 2, 0, 0])]
    #[case(2, 2, vec![])]
    #[case(1, 1, vec![1, 10, 9, 0, 0])]
    fn a_range_keeps_its_lines(#[case] start: u32, #[case] end: u32, #[case] expected: Vec<u32>) {
        let full = SemanticTokens {
            data: vec![1, 10, 9, 0, 0, 2, 4, 6, 0, 0, 0, 8, 2, 0, 0],
        };
        assert_eq!(tokens_in_lines(&full, start, end).data, expected);
    }
}
