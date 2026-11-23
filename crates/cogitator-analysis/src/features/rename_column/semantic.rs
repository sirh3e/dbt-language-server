//! Semantic models over a renamed model: the column inside an `expr:`
//! (SQL, read with the SQL lexer), and an entity, dimension or measure
//! that reads the column by its own `name:` alone, which gets an
//! `expr: <new>` beside it so its semantic name stays as it is. Metrics
//! name measures and dimensions, not columns, and exposures name nodes,
//! so neither changes.

use cogitator_dialects::Dialect;
use cogitator_lsp::{Position, Range};
use cogitator_parser::{DocumentParser, JinjaSqlParser, TokenKind};

use super::super::references::occur::{span, unquoted};
use super::yaml_lines::{block_end, indent, key_line, scalar_at};

/// One semantic site: a word to rename, or a line to add after `at`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SemanticSite {
    Word(Range),
    /// `name:` reads the column: the name's range, and where an
    /// `expr:` line goes with its indent.
    AddExpr {
        name: Range,
        at: Position,
        indent: usize,
    },
}

const ITEM_LISTS: [&str; 3] = ["entities", "dimensions", "measures"];

pub(super) fn semantic_sites(
    text: &str,
    dialect: &Dialect,
    renamed: &dyn Fn(&str) -> bool,
    column: &str,
) -> Vec<SemanticSite> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    let Some(top) = lines
        .iter()
        .position(|l| l.trim_end() == "semantic_models:")
    else {
        return out;
    };
    let end = block_end(&lines, top, 0);
    let mut no = top + 1;
    while no <= end {
        let line = lines[no];
        let entry = line.trim_start().starts_with("- ") && indent(line) <= 2;
        if !entry {
            no += 1;
            continue;
        }
        let last = block_end(&lines, no, indent(line));
        let model = (no..=last)
            .filter_map(|i| key_line(lines[i]))
            .find(|k| k.key == "model")
            .map(|k| k.value);
        if model.and_then(ref_name).is_some_and(|m| renamed(&m)) {
            items(&lines, no, last, dialect, column, &mut out);
        }
        no = last + 1;
    }
    out
}

fn ref_name(value: &str) -> Option<String> {
    let v: String = value.chars().filter(|c| !c.is_whitespace()).collect();
    let inner = v.strip_prefix("ref(")?.strip_suffix(')')?;
    Some(inner.trim_matches(['\'', '"']).to_string())
}

/// The entities, dimensions and measures of one semantic model.
fn items(
    lines: &[&str],
    first: usize,
    last: usize,
    dialect: &Dialect,
    column: &str,
    out: &mut Vec<SemanticSite>,
) {
    let mut list: Option<usize> = None;
    let mut i = first;
    while i <= last {
        let line = lines[i];
        let Some(k) = key_line(line) else {
            i += 1;
            continue;
        };
        if ITEM_LISTS.contains(&k.key) && !line.trim_start().starts_with("- ") {
            list = Some(k.at);
            i += 1;
            continue;
        }
        let in_list = list.is_some_and(|l| indent(line) > l);
        if !(in_list && line.trim_start().starts_with("- ")) {
            if k.key == "expr" {
                out.extend(expr_words(i, k.value, k.value_at, dialect, column));
            }
            i += 1;
            continue;
        }
        let item_end = block_end(lines, i, indent(line));
        let keys: Vec<_> = (i..=item_end)
            .filter_map(|j| key_line(lines[j]).map(|k| (j, k)))
            .collect();
        let expr = keys.iter().find(|(_, k)| k.key == "expr");
        match expr {
            Some((j, k)) => out.extend(expr_words(*j, k.value, k.value_at, dialect, column)),
            None => {
                if let Some((j, k)) = keys.iter().find(|(_, k)| k.key == "name") {
                    if let Some(name) = scalar_at(*j, k.value, k.value_at, column) {
                        out.push(SemanticSite::AddExpr {
                            name,
                            at: Position {
                                line: *j as u32,
                                character: lines[*j].len() as u32,
                            },
                            indent: k.at,
                        });
                    }
                }
            }
        }
        i = item_end + 1;
    }
}

/// The column's words in a SQL `expr:` value.
fn expr_words(
    line: usize,
    value: &str,
    value_at: usize,
    dialect: &Dialect,
    column: &str,
) -> Vec<SemanticSite> {
    let quoted = value.starts_with(['\'', '"']) && value.len() > 1;
    let sql = if quoted {
        &value[1..value.len() - 1]
    } else {
        value
    };
    let base = value_at + quoted as usize;
    let Ok(parsed) = JinjaSqlParser.parse(sql, dialect) else {
        return Vec::new();
    };
    parsed
        .tokens
        .iter()
        .filter(|t| matches!(t.kind, TokenKind::Word(_) | TokenKind::Ident) && t.line == 0)
        .filter(|t| unquoted(parsed.literal(t)).eq_ignore_ascii_case(column))
        .map(|t| SemanticSite::Word(span(line as u32, base as u32 + t.column, t.len())))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exprs_and_implicit_names() {
        let text = "semantic_models:\n  - name: orders\n    model: ref('orders')\n    entities:\n      - name: order_id\n        type: primary\n    dimensions:\n      - name: placed\n        expr: \"date_trunc('day', order_date)\"\n        type: time\n      - name: order_date\n        type: time\n    measures:\n      - name: total\n        expr: amount\n        agg: sum\n  - name: other\n    model: ref('customers')\n    dimensions:\n      - name: order_date\n";
        let renamed = |m: &str| m == "orders";
        let sites = semantic_sites(text, &Dialect::default(), &renamed, "order_date");
        assert_eq!(
            sites,
            vec![
                SemanticSite::Word(span(8, 33, 10)),
                SemanticSite::AddExpr {
                    name: span(10, 14, 10),
                    at: Position {
                        line: 10,
                        character: 24
                    },
                    indent: 8,
                },
            ]
        );
        let amount = semantic_sites(text, &Dialect::default(), &renamed, "amount");
        assert_eq!(amount, vec![SemanticSite::Word(span(14, 14, 6))]);
    }
}
