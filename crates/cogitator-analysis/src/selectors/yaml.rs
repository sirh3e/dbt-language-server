//! selectors.yml definitions (dbt-core `graph/cli.py`
//! `parse_from_definition`): a definition is a selector string, a method
//! mapping (`method`/`value` plus `parents`, `children`, their depths,
//! `childrens_parents`, `indirect_selection` and `exclude`, or the
//! `{tag: x}` shorthand), or a `union:`/`intersection:` list whose
//! `exclude:` element subtracts.

use cogitator_lsp::{Position, Range};
use cogitator_schemas::yaml::{as_str, entries, get, items, range_of, to_json};
use saphyr::{LoadableYamlNode, MarkedYaml, YamlData};
use serde_json::Value;

use super::eval::Indirect;
use super::method::Method;
use super::syntax::{self, Criterion};

pub use super::definition::{Definition, Expr, Problem, SelectorsFile};

/// Method-mapping keys dbt reads.
const METHOD_KEYS: &[&str] = &[
    "method",
    "value",
    "parents",
    "children",
    "parents_depth",
    "children_depth",
    "childrens_parents",
    "indirect_selection",
    "exclude",
];

pub fn parse(text: &str) -> SelectorsFile {
    let mut out = SelectorsFile::default();
    let Ok(docs) = MarkedYaml::load_from_str(text) else {
        return out;
    };
    let Some(top) = docs.first() else {
        return out;
    };
    let mut reader = Reader {
        text,
        problems: Vec::new(),
    };
    for entry in get(top, "selectors").into_iter().flat_map(items) {
        let Some(name_node) = get(entry, "name") else {
            continue;
        };
        let Some(name) = as_str(name_node) else {
            continue;
        };
        let range = range_of(name_node);
        if out.definitions.iter().any(|d| d.name == name) {
            reader.problem(range, format!("selector `{name}` is defined twice"));
        }
        let expr = match get(entry, "definition") {
            Some(def) => reader.expr(def),
            None => {
                reader.problem(range, format!("selector `{name}` has no `definition:`"));
                Expr::Invalid
            }
        };
        out.definitions.push(Definition {
            name: name.to_string(),
            description: get(entry, "description")
                .and_then(as_str)
                .unwrap_or_default()
                .to_string(),
            default: get(entry, "default").map(to_json) == Some(Value::Bool(true)),
            range,
            expr,
        });
    }
    let defaults: Vec<_> = out.definitions.iter().filter(|d| d.default).collect();
    if defaults.len() > 1 {
        for d in &defaults[1..] {
            reader.problem(
                d.range,
                "only one selector may set `default: true`".to_string(),
            );
        }
    }
    out.problems = reader.problems;
    out
}

struct Reader<'t> {
    text: &'t str,
    problems: Vec<Problem>,
}

impl Reader<'_> {
    fn problem(&mut self, range: Range, message: String) {
        self.problems.push(Problem { range, message });
    }

    fn expr(&mut self, node: &MarkedYaml<'_>) -> Expr {
        if let Some(text) = as_str(node) {
            return self.string(node, text, None);
        }
        let pairs: Vec<_> = entries(node).collect();
        if pairs.is_empty() {
            self.problem(
                range_of(node),
                "a definition is a string or a mapping".to_string(),
            );
            return Expr::Invalid;
        }
        for key in ["union", "intersection"] {
            if let Some(list) = get(node, key) {
                return self.set(list, key == "union");
            }
        }
        self.method(node)
    }

    /// `union:`/`intersection:` with at most one `exclude:` element.
    fn set(&mut self, list: &MarkedYaml<'_>, union: bool) -> Expr {
        let mut include = Vec::new();
        let mut exclude: Option<Expr> = None;
        for item in items(list) {
            match get(item, "exclude") {
                Some(excl) if entries(item).count() == 1 => {
                    if exclude.is_some() {
                        self.problem(
                            range_of(item),
                            "only one `exclude:` per union or intersection".to_string(),
                        );
                    }
                    exclude = Some(self.exclusions(excl));
                }
                _ => include.push(self.expr(item)),
            }
        }
        let base = if union {
            Expr::Union(include)
        } else {
            Expr::Intersection(include)
        };
        match exclude {
            Some(excl) => Expr::Difference(Box::new(base), Box::new(excl)),
            None => base,
        }
    }

    /// An `exclude:` value: a list (a union) or one definition.
    fn exclusions(&mut self, node: &MarkedYaml<'_>) -> Expr {
        match &node.data {
            YamlData::Sequence(_) => Expr::Union(items(node).map(|i| self.expr(i)).collect()),
            _ => self.expr(node),
        }
    }

    fn method(&mut self, node: &MarkedYaml<'_>) -> Expr {
        let pairs: Vec<_> = entries(node).collect();
        let (method_node, value_node) = match (get(node, "method"), get(node, "value")) {
            (Some(m), v) => (m, v),
            // `{tag: nightly}`: the one key is the method
            (None, _) if pairs.len() == 1 => (pairs[0].0, Some(pairs[0].1)),
            (None, _) => {
                self.problem(
                    range_of(node),
                    "a method mapping needs `method:` and `value:`".into(),
                );
                return Expr::Invalid;
            }
        };
        let shorthand = get(node, "method").is_none();
        if !shorthand {
            for (key, _) in &pairs {
                if let Some(k) = as_str(key).filter(|k| !METHOD_KEYS.contains(k)) {
                    self.problem(
                        range_of(key),
                        format!("unknown key `{k}` in a method mapping"),
                    );
                }
            }
        }
        let method_name = as_str(method_node).unwrap_or_default();
        let method_range = range_of(method_node);
        let Some(value_node) = value_node else {
            self.problem(
                method_range,
                format!("method `{method_name}` has no `value:`"),
            );
            return Expr::Invalid;
        };
        let value = match to_json(value_node) {
            Value::String(s) => s,
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            _ => {
                self.problem(range_of(value_node), "a method's value is a scalar".into());
                return Expr::Invalid;
            }
        };
        let flag = |key: &str| get(node, key).map(to_json) == Some(Value::Bool(true));
        let depth = |key: &str| {
            get(node, key)
                .map(to_json)
                .and_then(|v| v.as_u64())
                .map(|d| d as u32)
        };
        let parents_depth = depth("parents_depth");
        let children_depth = depth("children_depth");
        let criterion = Criterion {
            method: Method::parse(method_name),
            method_written: true,
            value,
            parents: (flag("parents") || parents_depth.is_some()).then_some(parents_depth),
            children: (flag("children") || children_depth.is_some()).then_some(children_depth),
            childrens_parents: flag("childrens_parents"),
            span: (0, 0),
        };
        let indirect = get(node, "indirect_selection").and_then(|n| {
            let parsed = as_str(n).and_then(Indirect::parse);
            if parsed.is_none() {
                self.problem(
                    range_of(n),
                    "indirect_selection is eager, cautious, buildable or empty".into(),
                );
            }
            parsed
        });
        let expr = Expr::Criterion {
            criterion,
            indirect,
            method_range,
            value_range: range_of(value_node),
        };
        match get(node, "exclude").filter(|_| !shorthand) {
            Some(excl) => Expr::Difference(Box::new(expr), Box::new(self.exclusions(excl))),
            None => expr,
        }
    }

    /// A string definition, in the `--select` syntax.
    fn string(&mut self, node: &MarkedYaml<'_>, text: &str, indirect: Option<Indirect>) -> Expr {
        let at = |offset: usize, len: usize| self.span_range(node, offset, len);
        match syntax::parse(text) {
            Ok(selection) => Expr::Union(
                selection
                    .union
                    .into_iter()
                    .map(|intersection| {
                        Expr::Intersection(
                            intersection
                                .into_iter()
                                .map(|criterion| {
                                    let range =
                                        at(criterion.span.0, criterion.span.1 - criterion.span.0);
                                    Expr::Criterion {
                                        criterion,
                                        indirect,
                                        method_range: range,
                                        value_range: range,
                                    }
                                })
                                .collect(),
                        )
                    })
                    .collect(),
            ),
            Err(err) => {
                let range = at(err.span.0, err.span.1 - err.span.0);
                self.problem(range, err.message);
                Expr::Invalid
            }
        }
    }

    /// The range of bytes `offset..offset+len` of a one-line scalar's
    /// value (past its opening quote), else the whole node.
    fn span_range(&self, node: &MarkedYaml<'_>, offset: usize, len: usize) -> Range {
        let whole = range_of(node);
        if whole.start.line != whole.end.line {
            return whole;
        }
        let line = self
            .text
            .lines()
            .nth(whole.start.line as usize)
            .unwrap_or_default();
        let start_col = whole.start.character as usize;
        let rest: String = line.chars().skip(start_col).collect();
        let quoted = rest.starts_with(['"', '\'']) as usize;
        let value = &rest[quoted..];
        let chars_before = value.get(..offset).map_or(0, |s| s.chars().count());
        let chars_len = value
            .get(offset..offset + len)
            .map_or(0, |s| s.chars().count());
        let start = (start_col + quoted + chars_before) as u32;
        Range {
            start: Position {
                line: whole.start.line,
                character: start,
            },
            end: Position {
                line: whole.start.line,
                character: start + chars_len as u32,
            },
        }
    }
}

#[cfg(test)]
#[path = "yaml_tests.rs"]
mod tests;
