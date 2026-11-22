//! The vendored properties-file schema walked along a YAML path: which
//! keys a mapping may hold, and which values a scalar may take, at any
//! depth (`models[].config`, `models[].columns[].data_tests[]`…). Read
//! from the same processed schema the typify macro compiles, so
//! completion never lists a key the deserializer does not know.

use serde_json::Value;

use crate::schema_keys::YML_FILES;

/// One step of a YAML path: a mapping key, or "an item of this list".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step<'a> {
    Key(&'a str),
    Item,
}

/// What a key's value is, so completion can open the right shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueShape {
    Scalar,
    Mapping,
    List,
}

/// One key a mapping may hold at a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaKey {
    pub name: String,
    /// The schema's description (or title); empty when it has none.
    pub doc: String,
    pub shape: ValueShape,
}

const MAX_DEPTH: usize = 24;

/// The keys of the mapping(s) a properties file allows at `path`,
/// sorted by name, `+`-prefixed spellings left out.
pub fn properties_keys_at(path: &[Step<'_>]) -> Vec<SchemaKey> {
    let nodes = nodes_at(&YML_FILES, path);
    let mut out: Vec<SchemaKey> = Vec::new();
    for node in &nodes {
        let Some(props) = node.get("properties").and_then(Value::as_object) else {
            continue;
        };
        for (name, schema) in props {
            if name.starts_with('+') || out.iter().any(|k| &k.name == name) {
                continue;
            }
            let expanded = expand(&YML_FILES, schema, 0);
            out.push(SchemaKey {
                name: name.clone(),
                doc: doc_of(&expanded),
                shape: shape_of(&expanded),
            });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// The literal values a scalar at `path` may take: its enums, and
/// `true`/`false` for a boolean. Empty for free text.
pub fn properties_values_at(path: &[Step<'_>]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for node in nodes_at(&YML_FILES, path) {
        if let Some(values) = node.get("enum").and_then(Value::as_array) {
            out.extend(values.iter().filter_map(Value::as_str).map(str::to_string));
        }
        if node.get("type").and_then(Value::as_str) == Some("boolean") {
            out.extend(["true".to_string(), "false".to_string()]);
        }
    }
    let mut seen = std::collections::HashSet::new();
    out.retain(|v| seen.insert(v.clone()));
    out
}

/// Every schema node the path can land on (unions flattened).
fn nodes_at<'a>(root: &'a Value, path: &[Step<'_>]) -> Vec<&'a Value> {
    let mut current = expand(root, root, 0);
    for step in path {
        let mut next = Vec::new();
        for node in &current {
            for child in children(node, *step) {
                for found in expand(root, child, 0) {
                    if !next.iter().any(|n: &&Value| std::ptr::eq(*n, found)) {
                        next.push(found);
                    }
                }
            }
        }
        if next.is_empty() {
            return Vec::new();
        }
        current = next;
    }
    current
}

fn children<'a>(node: &'a Value, step: Step<'_>) -> Vec<&'a Value> {
    match step {
        Step::Item => node.get("items").into_iter().collect(),
        Step::Key(key) => {
            let mut out = Vec::new();
            if let Some(child) = node.get("properties").and_then(|p| p.get(key)) {
                out.push(child);
            }
            if let Some(patterns) = node.get("patternProperties").and_then(Value::as_object) {
                out.extend(
                    patterns
                        .iter()
                        .filter(|(pattern, _)| matches(pattern, key))
                        .map(|(_, schema)| schema),
                );
            }
            if let Some(extra) = node.get("additionalProperties").filter(|v| v.is_object()) {
                out.push(extra);
            }
            out
        }
    }
}

fn matches(pattern: &str, key: &str) -> bool {
    regress::Regex::new(pattern).is_ok_and(|re| re.find(key).is_some())
}

/// A node with its `$ref` followed and its `anyOf`/`oneOf`/`allOf`
/// branches flattened next to it.
fn expand<'a>(root: &'a Value, node: &'a Value, depth: usize) -> Vec<&'a Value> {
    if depth > MAX_DEPTH {
        return Vec::new();
    }
    if let Some(target) = node.get("$ref").and_then(Value::as_str) {
        return resolve(root, target).map_or_else(Vec::new, |n| expand(root, n, depth + 1));
    }
    let mut out = vec![node];
    for union in ["anyOf", "oneOf", "allOf"] {
        if let Some(branches) = node.get(union).and_then(Value::as_array) {
            for branch in branches {
                out.extend(expand(root, branch, depth + 1));
            }
        }
    }
    out
}

/// A local JSON pointer (`#/$defs/x`).
fn resolve<'a>(root: &'a Value, target: &str) -> Option<&'a Value> {
    root.pointer(target.strip_prefix('#')?)
}

fn doc_of(nodes: &[&Value]) -> String {
    nodes
        .iter()
        .find_map(|n| {
            n.get("description")
                .or_else(|| n.get("title"))
                .and_then(Value::as_str)
        })
        .unwrap_or_default()
        .to_string()
}

fn shape_of(nodes: &[&Value]) -> ValueShape {
    let typed = |t: &str| {
        nodes
            .iter()
            .any(|n| n.get("type").and_then(Value::as_str) == Some(t))
    };
    let scalar = nodes.iter().any(|n| {
        n.get("enum").is_some()
            || matches!(
                n.get("type").and_then(Value::as_str),
                Some("string" | "number" | "integer" | "boolean")
            )
    });
    if typed("array") && !scalar {
        ValueShape::List
    } else if (typed("object") || nodes.iter().any(|n| n.get("properties").is_some())) && !scalar {
        ValueShape::Mapping
    } else {
        ValueShape::Scalar
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn names(path: &[Step<'_>]) -> Vec<String> {
        properties_keys_at(path)
            .into_iter()
            .map(|k| k.name)
            .collect()
    }

    #[rstest]
    #[case::root(&[], &["models", "sources", "version"])]
    #[case::model(&[Step::Key("models"), Step::Item], &["columns", "config", "data_tests", "name"])]
    #[case::model_config(
        &[Step::Key("models"), Step::Item, Step::Key("config")],
        &["contract", "event_time", "materialized"]
    )]
    #[case::column(
        &[Step::Key("models"), Step::Item, Step::Key("columns"), Step::Item],
        &["data_tests", "data_type", "description", "name"]
    )]
    #[case::test_item(
        &[Step::Key("models"), Step::Item, Step::Key("columns"), Step::Item, Step::Key("data_tests"), Step::Item],
        &["accepted_values", "relationships"]
    )]
    #[case::relationships(
        &[Step::Key("models"), Step::Item, Step::Key("columns"), Step::Item, Step::Key("data_tests"), Step::Item, Step::Key("relationships")],
        &["config", "field", "to", "where"]
    )]
    #[case::accepted_values(
        &[Step::Key("models"), Step::Item, Step::Key("data_tests"), Step::Item, Step::Key("accepted_values")],
        &["quote", "values"]
    )]
    #[case::custom_test(
        &[Step::Key("models"), Step::Item, Step::Key("data_tests"), Step::Item, Step::Key("my_test")],
        &["config", "name", "where"]
    )]
    #[case::source_table(
        &[Step::Key("sources"), Step::Item, Step::Key("tables"), Step::Item],
        &["columns", "identifier", "name"]
    )]
    fn keys_follow_refs_and_unions(#[case] path: &[Step<'_>], #[case] expected: &[&str]) {
        let got = names(path);
        for key in expected {
            assert!(got.iter().any(|k| k == key), "{key} missing from {got:?}");
        }
    }

    #[test]
    fn a_path_the_schema_lacks_has_no_keys() {
        assert!(names(&[Step::Key("models"), Step::Key("bogus")]).is_empty());
        assert!(!names(&[Step::Key("models"), Step::Item])
            .iter()
            .any(|k| k.starts_with('+')));
    }

    #[rstest]
    #[case::access(&[Step::Key("models"), Step::Item, Step::Key("access")], "public")]
    #[case::granularity(
        &[Step::Key("models"), Step::Item, Step::Key("columns"), Step::Item, Step::Key("granularity")],
        "day"
    )]
    #[case::quote_flag(
        &[Step::Key("models"), Step::Item, Step::Key("data_tests"), Step::Item, Step::Key("accepted_values"), Step::Key("quote")],
        "true"
    )]
    fn values_are_enums_and_booleans(#[case] path: &[Step<'_>], #[case] expected: &str) {
        let got = properties_values_at(path);
        assert!(got.iter().any(|v| v == expected), "{got:?}");
    }

    #[test]
    fn shapes_open_the_right_value() {
        let model = properties_keys_at(&[Step::Key("models"), Step::Item]);
        let shape = |n: &str| model.iter().find(|k| k.name == n).map(|k| k.shape);
        assert_eq!(shape("columns"), Some(ValueShape::List));
        assert_eq!(shape("config"), Some(ValueShape::Mapping));
        assert_eq!(shape("name"), Some(ValueShape::Scalar));
    }
}
