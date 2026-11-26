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
pub(crate) enum SemanticSite {
    Word(Range),
    /// `name:` reads the column: the name's range, and where an
    /// `expr:` line goes with its indent.
    AddExpr {
        name: Range,
        at: Position,
        indent: usize,
    },
}

/// Which list of a semantic model an item sits in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SemanticList {
    Entity,
    Dimension,
    Measure,
}

/// One place of a semantic model that reads the column: the site, and
/// the item it belongs to (`list`/`name` are `None` for an `expr:` that
/// is no list item's).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SemanticItem {
    pub semantic_model: String,
    pub model: String,
    pub list: Option<SemanticList>,
    pub name: Option<String>,
    pub site: SemanticSite,
}

const ITEM_LISTS: [(&str, SemanticList); 3] = [
    ("entities", SemanticList::Entity),
    ("dimensions", SemanticList::Dimension),
    ("measures", SemanticList::Measure),
];

pub(super) fn semantic_sites(
    text: &str,
    dialect: &Dialect,
    renamed: &dyn Fn(&str) -> bool,
    column: &str,
) -> Vec<SemanticSite> {
    semantic_items(text, dialect, renamed, column)
        .into_iter()
        .map(|item| item.site)
        .collect()
}

/// Every entity, dimension and measure (and stray `expr:`) of the
/// semantic models over a model `over` accepts that reads `column`:
/// its `expr:` names the column, or it has no `expr:` and its `name:` is
/// the column. Shared by the column rename and the column reach.
pub(crate) fn semantic_items(
    text: &str,
    dialect: &Dialect,
    over: &dyn Fn(&str) -> bool,
    column: &str,
) -> Vec<SemanticItem> {
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
        let entry_at = key_line(line).map_or(indent(line) + 2, |k| k.at);
        let own = |key: &str| {
            (no..=last)
                .filter_map(|i| key_line(lines[i]))
                .find(|k| k.key == key && k.at == entry_at)
                .map(|k| k.value)
        };
        let model = own("model").and_then(ref_name);
        if let Some(model) = model.filter(|m| over(m)) {
            let name = own("name").unwrap_or_default();
            let owner = Owner {
                semantic_model: name.trim_matches(['\'', '"']),
                model: &model,
            };
            items(&lines, no, last, dialect, column, &owner, &mut out);
        }
        no = last + 1;
    }
    out
}

struct Owner<'a> {
    semantic_model: &'a str,
    model: &'a str,
}

impl Owner<'_> {
    fn item(
        &self,
        list: Option<SemanticList>,
        name: Option<&str>,
        site: SemanticSite,
    ) -> SemanticItem {
        SemanticItem {
            semantic_model: self.semantic_model.to_string(),
            model: self.model.to_string(),
            list,
            name: name.map(|n| n.trim_matches(['\'', '"']).to_string()),
            site,
        }
    }
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
    owner: &Owner<'_>,
    out: &mut Vec<SemanticItem>,
) {
    let mut list: Option<(usize, SemanticList)> = None;
    let mut i = first;
    while i <= last {
        let line = lines[i];
        let Some(k) = key_line(line) else {
            i += 1;
            continue;
        };
        if let Some((_, kind)) = ITEM_LISTS.iter().find(|(key, _)| *key == k.key) {
            if !line.trim_start().starts_with("- ") {
                list = Some((k.at, *kind));
                i += 1;
                continue;
            }
        }
        let in_list = list.filter(|(at, _)| indent(line) > *at);
        let Some((_, kind)) = in_list.filter(|_| line.trim_start().starts_with("- ")) else {
            if k.key == "expr" {
                let words = expr_words(i, k.value, k.value_at, dialect, column);
                out.extend(words.into_iter().map(|w| owner.item(None, None, w)));
            }
            i += 1;
            continue;
        };
        let item_end = block_end(lines, i, indent(line));
        let keys: Vec<_> = (i..=item_end)
            .filter_map(|j| key_line(lines[j]).map(|k| (j, k)))
            .collect();
        let name = keys.iter().find(|(_, k)| k.key == "name");
        let item_name = name.map(|(_, k)| k.value);
        match keys.iter().find(|(_, k)| k.key == "expr") {
            Some((j, k)) => {
                let words = expr_words(*j, k.value, k.value_at, dialect, column);
                out.extend(
                    words
                        .into_iter()
                        .map(|w| owner.item(Some(kind), item_name, w)),
                );
            }
            None => {
                if let Some((j, k)) = name {
                    if let Some(range) = scalar_at(*j, k.value, k.value_at, column) {
                        let site = SemanticSite::AddExpr {
                            name: range,
                            at: Position {
                                line: *j as u32,
                                character: lines[*j].len() as u32,
                            },
                            indent: k.at,
                        };
                        out.push(owner.item(Some(kind), item_name, site));
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

    #[test]
    fn items_name_their_list_and_model() {
        let text = "semantic_models:\n  - name: orders_sm\n    model: ref('orders')\n    measures:\n      - name: total\n        expr: amount * 2\n        agg: sum\n      - name: amount\n        agg: sum\n    dimensions:\n      - name: status\n";
        let over = |m: &str| m == "orders";
        let got: Vec<_> = semantic_items(text, &Dialect::default(), &over, "amount")
            .into_iter()
            .map(|i| (i.semantic_model, i.model, i.list, i.name))
            .collect();
        let item = |name: &str| {
            (
                "orders_sm".to_string(),
                "orders".to_string(),
                Some(SemanticList::Measure),
                Some(name.to_string()),
            )
        };
        assert_eq!(got, vec![item("total"), item("amount")]);
    }
}
