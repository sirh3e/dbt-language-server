//! dbt names in properties YAML that the Jinja pipeline does not see:
//! plain-scalar `ref('x')` / `source('a', 'b')` (relationship tests'
//! `to:`, exposures' `depends_on:`) and `macros:` entries.
//!
//! No lexer of our own: each candidate scalar is wrapped as `{{ … }}` and
//! run through the same refinement as a template, then the token columns
//! are mapped back onto the file and verified against its text.

use std::path::Path;

use cogitator_dialects::Dialect;
use cogitator_lsp::{Position, Range};
use cogitator_parser::{DocumentParser, JinjaSqlParser, TokenKind};
use cogitator_schemas::yaml;
use saphyr::{MarkedYaml, YamlData};

use super::occur::{name_span, span};

/// One dbt name found in a plain YAML scalar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct YamlCall {
    /// `Ref`, `Package`, `SourceName` or `SourceTable`.
    pub kind: TokenKind,
    pub literal: String,
    pub range: Range,
    /// The source a `SourceTable` belongs to.
    pub source: Option<String>,
}

const PREFIX: u32 = 3; // "{{ "

/// Every plain-scalar ref()/source() argument of a YAML file.
pub(crate) fn yaml_calls(text: &str, dialect: &Dialect) -> Vec<YamlCall> {
    let Ok(root) = yaml::load_marked(text, Path::new("")) else {
        return Vec::new();
    };
    let mut scalars = Vec::new();
    collect_scalars(&root, &mut scalars);
    let lines: Vec<&str> = text.lines().collect();
    scalars
        .into_iter()
        .flat_map(|(value, pos)| calls_in(value, pos, &lines, dialect))
        .collect()
}

fn collect_scalars<'a>(node: &'a MarkedYaml<'_>, out: &mut Vec<(&'a str, (u32, usize))>) {
    match &node.data {
        YamlData::Mapping(map) => map.iter().for_each(|(_, v)| collect_scalars(v, out)),
        YamlData::Sequence(seq) => seq.iter().for_each(|v| collect_scalars(v, out)),
        YamlData::Tagged(_, inner) => collect_scalars(inner, out),
        _ => {
            let Some(value) = yaml::as_str(node) else {
                return;
            };
            let single_line = node.span.start.line() == node.span.end.line();
            let candidate = value.contains("ref(") || value.contains("source(");
            if single_line && candidate && !value.contains("{{") {
                let line = node.span.start.line().saturating_sub(1) as u32;
                out.push((value, (line, node.span.start.col())));
            }
        }
    }
}

fn calls_in(
    value: &str,
    (line, char_col): (u32, usize),
    lines: &[&str],
    dialect: &Dialect,
) -> Vec<YamlCall> {
    let Some(text_line) = lines.get(line as usize) else {
        return Vec::new();
    };
    // saphyr counts characters; tokens count bytes
    let byte_col = text_line
        .char_indices()
        .nth(char_col)
        .map_or(text_line.len(), |(b, _)| b);
    let quoted = text_line[byte_col..].starts_with(['\'', '"']);
    let base = (byte_col + usize::from(quoted)) as u32;
    let wrapped = format!("{{{{ {value} }}}}");
    let Ok(parsed) = JinjaSqlParser.parse(&wrapped, dialect) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut source: Option<String> = None;
    for t in &parsed.tokens {
        let kind = t.kind;
        if !matches!(
            kind,
            TokenKind::Ref | TokenKind::Package | TokenKind::SourceName | TokenKind::SourceTable
        ) || t.line != 0
        {
            continue;
        }
        let literal = parsed.literal(t).to_string();
        let character = base + t.column - PREFIX;
        let at = character as usize;
        if text_line.get(at..at + literal.len()) != Some(literal.as_str()) {
            continue; // an escape moved the text: no reliable range
        }
        if kind == TokenKind::SourceName {
            source = Some(literal.clone());
        }
        out.push(YamlCall {
            kind,
            range: span(line, character, literal.len() as u32),
            source: (kind == TokenKind::SourceTable)
                .then(|| source.clone())
                .flatten(),
            literal,
        });
    }
    out
}

/// The `name:` value ranges of `macros:` entries naming `name`.
pub(crate) fn macro_entries(text: &str, name: &str) -> Vec<Range> {
    let Ok(root) = yaml::load_marked(text, Path::new("")) else {
        return Vec::new();
    };
    let Some(macros) = yaml::get(&root, "macros") else {
        return Vec::new();
    };
    yaml::items(macros)
        .filter_map(|entry| yaml::get(entry, "name"))
        .filter(|n| yaml::as_str(n) == Some(name))
        .filter_map(|n| {
            let p = yaml::position_of(n);
            name_span(
                text,
                Position {
                    line: p.line,
                    character: p.character,
                },
                name,
            )
        })
        .collect()
}

/// The `macros:` entry name under the cursor.
pub(crate) fn macro_entry_at(text: &str, pos: Position) -> Option<String> {
    let root = yaml::load_marked(text, Path::new("")).ok()?;
    let macros = yaml::get(&root, "macros")?;
    let hit = yaml::items(macros)
        .filter_map(|entry| yaml::get(entry, "name"))
        .filter_map(|n| {
            let name = yaml::as_str(n)?;
            let range = name_span(text, yaml::position_of(n), name)?;
            super::occur::contains(&range, pos).then(|| name.to_string())
        })
        .next();
    hit
}

#[cfg(test)]
mod tests {
    use super::*;

    const YAML: &str = "models:
  - name: orders
    columns:
      - name: customer_id
        data_tests:
          - relationships:
              to: ref('customers')
              field: id
          - relationships:
              to: \"ref('pkg', 'stg_x')\"
      - name: c
        description: '{{ doc(\"x\") }} ref(\"ignored\")'
exposures:
  - name: e
    depends_on:
      - source('stripe', 'payments')
macros:
  - name: full_name
    description: x
  - name: 'other'
";

    #[test]
    fn plain_scalars_yield_their_dbt_names() {
        let calls = yaml_calls(YAML, &Dialect::default());
        let got: Vec<(TokenKind, &str, u32, u32)> = calls
            .iter()
            .map(|c| {
                (
                    c.kind,
                    c.literal.as_str(),
                    c.range.start.line,
                    c.range.start.character,
                )
            })
            .collect();
        assert_eq!(
            got,
            vec![
                (TokenKind::Ref, "customers", 6, 23),
                (TokenKind::Package, "pkg", 9, 24),
                (TokenKind::Ref, "stg_x", 9, 31),
                (TokenKind::SourceName, "stripe", 15, 16),
                (TokenKind::SourceTable, "payments", 15, 26),
            ]
        );
        let table = calls.last().unwrap();
        assert_eq!(table.source.as_deref(), Some("stripe"));
        // every range spells its literal in the file
        let lines: Vec<&str> = YAML.lines().collect();
        for c in &calls {
            let line = lines[c.range.start.line as usize];
            let (a, b) = (
                c.range.start.character as usize,
                c.range.end.character as usize,
            );
            assert_eq!(&line[a..b], c.literal);
        }
    }

    #[test]
    fn macro_entries_and_the_entry_under_the_cursor() {
        let ranges = macro_entries(YAML, "full_name");
        assert_eq!(ranges.len(), 1);
        assert_eq!((ranges[0].start.line, ranges[0].start.character), (17, 10));
        assert_eq!(macro_entries(YAML, "other")[0].start.character, 11);
        assert!(macro_entries(YAML, "nope").is_empty());
        let at = |line, character| Position { line, character };
        for (pos, expected) in [
            (at(17, 12), Some("full_name")),
            (at(19, 12), Some("other")),
            (at(18, 6), None),
            (at(1, 10), None),
        ] {
            assert_eq!(macro_entry_at(YAML, pos).as_deref(), expected, "{pos:?}");
        }
        assert!(macro_entries("not: [yaml", "x").is_empty());
    }
}
