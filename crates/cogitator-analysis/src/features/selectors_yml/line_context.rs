//! Where the cursor is in a YAML file, as far as selector completion
//! cares: read from the line text like the other line-wise YAML helpers
//! (`yaml_text.rs`), since a half-typed value often does not parse.

use crate::selectors::Method;

/// What the cursor is typing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Slot {
    /// A `method:` value.
    Method { prefix: String },
    /// A value of a known method (`value:` beside `method:`, the
    /// `{tag: x}` shorthand, or after `tag:` in a string).
    Value { method: Method, prefix: String },
    /// A string-form word with no method yet: a method or a node name.
    Word { prefix: String },
    /// A tag value of `tags:`/`+tags:`.
    Tag { prefix: String },
}

/// (indentation of the key, key, rest after `key:`) of a `key: value`
/// line, a leading `- ` counted as indentation.
fn key_line(line: &str) -> Option<(usize, &str, &str)> {
    let body = line.trim_start();
    let mut col = line.len() - body.len();
    let body = match body.strip_prefix("- ") {
        Some(rest) => {
            col += 2 + (rest.len() - rest.trim_start().len());
            rest.trim_start()
        }
        None => body,
    };
    let colon = body.find(':')?;
    let key = &body[..colon];
    let rest = &body[colon + 1..];
    let is_key = !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '+' | '-'))
        && (rest.is_empty() || rest.starts_with(' '));
    is_key.then_some((col, key, rest.trim_start()))
}

/// The slot at (`line`, `character`) of a selectors.yml text.
pub fn selectors_slot(text: &str, line: usize, character: usize) -> Option<Slot> {
    let lines: Vec<&str> = text.split('\n').collect();
    let current = lines.get(line)?;
    let prefix = current.get(..character.min(current.len()))?;
    if let Some((col, key, rest)) = key_line(prefix) {
        let typed = unquote(rest).to_string();
        return match key {
            "method" => Some(Slot::Method { prefix: typed }),
            "value" => {
                let method = sibling_method(&lines, line, col)?;
                Some(Slot::Value {
                    method,
                    prefix: typed,
                })
            }
            "definition" => Some(string_slot(rest)),
            "name" | "description" | "default" | "union" | "intersection" | "exclude"
            | "selectors" | "parents" | "children" | "parents_depth" | "children_depth"
            | "childrens_parents" | "indirect_selection" => None,
            other => match Method::parse(other) {
                Method::Unknown(_) => None,
                method => Some(Slot::Value {
                    method,
                    prefix: typed,
                }),
            },
        };
    }
    // a scalar list item: the string form
    let item = prefix.trim_start().strip_prefix('-')?;
    Some(string_slot(item.trim_start()))
}

/// The slot of the last word of a string-form selector being typed.
fn string_slot(typed: &str) -> Slot {
    let typed = unquote(typed);
    let word = typed
        .rsplit(|c: char| c.is_whitespace() || c == ',')
        .next()
        .unwrap_or_default();
    let word = word.trim_start_matches('@');
    let word = word.trim_start_matches(|c: char| c.is_ascii_digit() || c == '+');
    match word.split_once(':') {
        Some((method, value)) => Slot::Value {
            method: Method::parse(method),
            prefix: value.to_string(),
        },
        None => Slot::Word {
            prefix: word.to_string(),
        },
    }
}

fn unquote(text: &str) -> &str {
    text.trim_start_matches(['"', '\''])
}

/// The `method:` of the mapping a `value:` at `col` sits in: its lines
/// above up to the one opening the list item, and below up to the next.
fn sibling_method(lines: &[&str], line: usize, col: usize) -> Option<Method> {
    let starts_item = |l: &str| l.trim_start().starts_with("- ");
    let column = |l: &str| key_line(l).map_or(l.len() - l.trim_start().len(), |(c, _, _)| c);
    let mut block = Vec::new();
    if !starts_item(lines[line]) {
        for l in lines[..line].iter().rev() {
            if l.trim().is_empty() {
                continue;
            }
            if column(l) < col {
                break;
            }
            block.push(*l);
            if starts_item(l) && column(l) == col {
                break;
            }
        }
    }
    for l in &lines[line + 1..] {
        if l.trim().is_empty() {
            continue;
        }
        if column(l) < col || starts_item(l) {
            break;
        }
        block.push(*l);
    }
    block
        .into_iter()
        .filter_map(key_line)
        .find(|(c, key, _)| *c == col && *key == "method")
        .map(|(_, _, value)| Method::parse(value.trim_matches(['"', '\''])))
}

/// The slot of a `tags:` value in dbt_project.yml or a properties file.
pub fn tag_slot(text: &str, line: usize, character: usize) -> Option<Slot> {
    let lines: Vec<&str> = text.split('\n').collect();
    let current = lines.get(line)?;
    let prefix = current.get(..character.min(current.len()))?;
    if let Some((_, key, rest)) = key_line(prefix) {
        if key == "tags" || key == "+tags" {
            let word = rest.rsplit(['[', ',', ' ']).next().unwrap_or_default();
            return Some(Slot::Tag {
                prefix: unquote(word).to_string(),
            });
        }
        return None;
    }
    let item = prefix.trim_start().strip_prefix('-')?;
    let indent = current.len() - current.trim_start().len();
    let parent = lines[..line].iter().rev().find(|l| {
        let i = l.len() - l.trim_start().len();
        !l.trim().is_empty() && (i < indent || (i == indent && !l.trim_start().starts_with('-')))
    })?;
    let (_, key, rest) = key_line(parent)?;
    ((key == "tags" || key == "+tags") && rest.is_empty()).then(|| Slot::Tag {
        prefix: unquote(item.trim_start()).to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SELECTORS: &str = "\
selectors:
  - name: a
    definition:
      method: tag
      value: ni
  - name: b
    definition: '+tag:x pa'
  - name: c
    definition:
      union:
        - 'config.materialized:t'
        - method: path
          value: mod
        - package: jaff
";

    #[test]
    fn selectors_slots() {
        let cases: &[(usize, usize, Option<Slot>)] = &[
            (
                3,
                18,
                Some(Slot::Method {
                    prefix: "tag".into(),
                }),
            ),
            (
                4,
                15,
                Some(Slot::Value {
                    method: Method::Tag,
                    prefix: "ni".into(),
                }),
            ),
            (
                6,
                26,
                Some(Slot::Word {
                    prefix: "pa".into(),
                }),
            ),
            (
                6,
                23,
                Some(Slot::Value {
                    method: Method::Tag,
                    prefix: "x".into(),
                }),
            ),
            (
                10,
                32,
                Some(Slot::Value {
                    method: Method::Config("materialized".into()),
                    prefix: "t".into(),
                }),
            ),
            (
                12,
                20,
                Some(Slot::Value {
                    method: Method::Path,
                    prefix: "mod".into(),
                }),
            ),
            (
                13,
                23,
                Some(Slot::Value {
                    method: Method::Package,
                    prefix: "jaff".into(),
                }),
            ),
            (1, 11, None),
        ];
        for (line, character, expected) in cases {
            assert_eq!(
                &selectors_slot(SELECTORS, *line, *character),
                expected,
                "{line}:{character}"
            );
        }
    }

    #[test]
    fn tag_slots() {
        let text = "models:\n  - name: m\n    tags: [nightly, ma\n    config:\n      tags:\n        - hou\nmodels2:\n  p:\n    +tags: da\n";
        let cases: &[(usize, usize, Option<&str>)] = &[
            (2, 22, Some("ma")),
            (5, 13, Some("hou")),
            (8, 13, Some("da")),
            (1, 11, None),
        ];
        for (line, character, expected) in cases {
            let got = tag_slot(text, *line, *character).map(|s| match s {
                Slot::Tag { prefix } => prefix,
                other => panic!("{other:?}"),
            });
            assert_eq!(got.as_deref(), *expected, "{line}:{character}");
        }
    }
}
