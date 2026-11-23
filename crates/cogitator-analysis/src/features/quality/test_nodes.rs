//! The tests a properties YAML defines as dbt nodes, for node selection:
//! each generic test with its dbt name and what it is attached to, and
//! each unit test with its model. `relationships`-style tests name a
//! second parent in an argument (`to: ref('x')`), which dbt makes a
//! parent too; those are read out of the arguments.

use cogitator_lsp::Range;
use cogitator_schemas::yaml::{as_str, get, items, position_of};
use saphyr::{LoadableYamlNode, MarkedYaml};

use super::py_yaml::Source;
use super::test_names::read_test_entry;
use super::test_scope::applied_tests;

/// What a test hangs off: a model-like node by name, or a source table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestParent {
    Node(String),
    /// `<source>_<table>` as the YAML writes it: which source and table
    /// the caller resolves, as `_` may sit in either name.
    SourceTarget(String),
    Source(String, String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YamlTest {
    /// The dbt node name (`not_null_orders_order_id`, a unit test's name).
    pub name: String,
    /// The generic test as written (`not_null`, `dbt_utils.accepted_range`);
    /// empty for a unit test.
    pub test: String,
    pub unit: bool,
    pub column: Option<String>,
    pub parents: Vec<TestParent>,
    pub range: Range,
}

/// Every test node of a properties YAML text, in file order.
pub fn yaml_test_nodes(text: &str) -> Vec<YamlTest> {
    let Ok(docs) = MarkedYaml::load_from_str(text) else {
        return Vec::new();
    };
    let Some(root) = docs.first() else {
        return Vec::new();
    };
    let src = Source::new(text);
    let mut out = Vec::new();
    for applied in applied_tests(root, &src) {
        let Some(entry) = read_test_entry(applied.item, &src) else {
            continue;
        };
        let name = entry.name(&applied.target, applied.is_source, applied.column);
        let mut parents = vec![if applied.is_source {
            TestParent::SourceTarget(applied.target.clone())
        } else {
            TestParent::Node(applied.target.clone())
        }];
        for (_, parts) in &entry.args {
            for part in parts {
                parents.extend(referenced(part));
            }
        }
        parents.dedup();
        out.push(YamlTest {
            name,
            test: entry.test.clone(),
            unit: false,
            column: applied.column.map(str::to_string),
            parents,
            range: Range::zero_width(position_of(applied.item)),
        });
    }
    for unit in get(root, "unit_tests").into_iter().flat_map(items) {
        let (Some(name_node), Some(model)) =
            (get(unit, "name"), get(unit, "model").and_then(as_str))
        else {
            continue;
        };
        let Some(name) = as_str(name_node) else {
            continue;
        };
        out.push(YamlTest {
            name: name.to_string(),
            test: String::new(),
            unit: true,
            column: None,
            parents: vec![TestParent::Node(model.to_string())],
            range: Range::zero_width(position_of(name_node)),
        });
    }
    out
}

/// `ref('x')` / `source('a', 'b')` inside an argument's text.
fn referenced(text: &str) -> Vec<TestParent> {
    let mut out = Vec::new();
    let quoted = |s: &str| -> Vec<String> {
        s.split(',')
            .map(|p| p.trim().trim_matches(|c| c == '\'' || c == '"').to_string())
            .filter(|p| !p.is_empty())
            .collect()
    };
    for (call, make) in [("ref(", 0), ("source(", 1)] {
        let mut rest = text;
        while let Some(at) = rest.find(call) {
            let after = &rest[at + call.len()..];
            let Some(close) = after.find(')') else {
                break;
            };
            let args = quoted(&after[..close]);
            match (make, args.as_slice()) {
                // ref('package', 'model') names the model last
                (0, [.., model]) => out.push(TestParent::Node(model.clone())),
                (1, [source, table]) => out.push(TestParent::Source(source.clone(), table.clone())),
                _ => {}
            }
            rest = &after[close..];
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn generic_tests_name_their_parents_and_unit_tests_their_model() {
        let text = "version: 2\nmodels:\n  - name: orders\n    columns:\n      - name: customer_id\n        data_tests:\n          - not_null\n          - relationships:\n              arguments:\n                to: ref('customers')\n                field: customer_id\nsources:\n  - name: raw\n    tables:\n      - name: items\n        columns:\n          - name: id\n            data_tests: [unique]\nunit_tests:\n  - name: test_orders_amounts\n    model: orders\n";
        let tests = yaml_test_nodes(text);
        let shape: Vec<(&str, Vec<TestParent>)> = tests
            .iter()
            .map(|t| (t.name.as_str(), t.parents.clone()))
            .collect();
        assert_eq!(
            shape,
            vec![
                (
                    "not_null_orders_customer_id",
                    vec![TestParent::Node("orders".into())]
                ),
                (
                    "relationships_orders_customer_id__customer_id__ref_customers_",
                    vec![
                        TestParent::Node("orders".into()),
                        TestParent::Node("customers".into())
                    ]
                ),
                (
                    "source_unique_raw_items_id",
                    vec![TestParent::SourceTarget("raw_items".into())]
                ),
                (
                    "test_orders_amounts",
                    vec![TestParent::Node("orders".into())]
                ),
            ]
        );
        assert!(tests[3].unit);
        assert_eq!(tests[0].test, "not_null");
    }
}
