//! "Run this test" lenses: one per generic test entry of a properties
//! YAML (model, seed, snapshot and source tests, per column, per
//! version), one per unit test, and one on a singular test file. Each
//! runs `dbt test --select <that test's dbt name>` through
//! dbt.runSelection, so exactly that test runs. A test entry on a
//! versioned model's base entry is one dbt test per version it applies
//! to; its lens selects all of them (`a b`, dbt's space union).

use cogitator_lsp::{CodeLens, Command, Range};
use cogitator_schemas::yaml::{as_str, get, items, position_of};
use saphyr::{LoadableYamlNode, MarkedYaml};
use serde::Serialize;

use super::py_yaml::Source;
use super::test_names::read_test_entry;
use super::test_scope::applied_tests;
use crate::graph::NodeKind;
use crate::DbtContext;

/// One test entry the YAML defines, with the dbt names of the tests it
/// becomes and the selector that picks exactly those.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestTarget {
    pub selector: String,
    pub names: Vec<String>,
    pub unit: bool,
    pub range: Range,
}

#[derive(Debug, Serialize)]
struct RunSelectionArgument<'a> {
    uri: &'a str,
    selector: &'a str,
    kind: &'static str,
}

/// Every test of a properties YAML text, in file order.
pub fn yaml_tests(text: &str) -> Vec<TestTarget> {
    let Ok(docs) = MarkedYaml::load_from_str(text) else {
        return Vec::new();
    };
    let Some(root) = docs.first() else {
        return Vec::new();
    };
    let src = Source::new(text);
    let mut out: Vec<TestTarget> = Vec::new();
    for applied in applied_tests(root, &src) {
        let Some(entry) = read_test_entry(applied.item, &src) else {
            continue;
        };
        let name = entry.name(&applied.target, applied.is_source, applied.column);
        let range = Range::zero_width(position_of(applied.item));
        match out.iter_mut().find(|t| t.range == range) {
            Some(t) if !t.names.contains(&name) => t.names.push(name),
            Some(_) => {}
            None => out.push(TestTarget {
                selector: String::new(),
                names: vec![name],
                unit: false,
                range,
            }),
        }
    }
    for t in &mut out {
        t.selector = crate::selectors::union_of(&t.names);
    }
    for unit in get(root, "unit_tests").into_iter().flat_map(items) {
        if let Some(name_node) = get(unit, "name") {
            if let Some(name) = as_str(name_node) {
                out.push(TestTarget {
                    selector: name.to_string(),
                    names: vec![name.to_string()],
                    unit: true,
                    range: Range::zero_width(position_of(name_node)),
                });
            }
        }
    }
    out.sort_by_key(|t| (t.range.start.line, t.range.start.character));
    out
}

/// The lenses over a properties YAML's tests.
pub fn yaml_test_lenses(uri: &str, text: &str) -> Vec<CodeLens> {
    yaml_tests(text)
        .into_iter()
        .map(|t| {
            let title = match (t.unit, t.names.len()) {
                (true, _) => "Run this unit test".to_string(),
                (false, 1) => "Run this test".to_string(),
                (false, n) => format!("Run this test ({n} versions)"),
            };
            run_lens(uri, t.range, &title, &t.selector)
        })
        .collect()
}

/// On a singular test file (test-paths): run it, by its file stem.
pub fn singular_test_lens(ctx: &DbtContext, uri: &str) -> Option<CodeLens> {
    let path = cogitator_lsp::uri_to_fs_path(uri).unwrap_or_else(|| uri.to_string());
    let path = std::path::Path::new(&path);
    let stem = path.file_name()?.to_str()?.strip_suffix(".sql")?;
    let node = ctx.graph.node(NodeKind::Test, stem)?;
    // Path equality compares components, so `/` and `\` separators match
    (node.uri.as_path() == path).then(|| run_lens(uri, Range::default(), "Run this test", stem))
}

fn run_lens(uri: &str, range: Range, title: &str, selector: &str) -> CodeLens {
    let arg = serde_json::to_value(RunSelectionArgument {
        uri,
        selector,
        kind: "test",
    })
    .expect("lens argument serializes");
    CodeLens {
        range,
        command: Command {
            title: title.to_string(),
            command: "dbt.runSelection".to_string(),
            arguments: vec![arg],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const YAML: &str = "\
version: 2
models:
  - name: orders
    data_tests:
      - dbt_utils.expression_is_true:
          expression: \"amount >= 0\"
    columns:
      - name: order_id
        data_tests:
          - unique
          - not_null
      - name: status
        tests:
          - accepted_values:
              values: ['placed', 'shipped']
sources:
  - name: raw
    tables:
      - name: orders
        columns:
          - name: id
            data_tests: [unique]
unit_tests:
  - name: orders_unit
    model: orders
";

    #[test]
    fn every_test_gets_its_selector() {
        let selectors: Vec<(String, bool, u32)> = yaml_tests(YAML)
            .into_iter()
            .map(|t| (t.selector, t.unit, t.range.start.line))
            .collect();
        let expect = |s: &str, unit, line| (s.to_string(), unit, line);
        assert_eq!(
            selectors,
            vec![
                expect("dbt_utils_expression_is_true_orders_amount_0", false, 4),
                expect("unique_orders_order_id", false, 9),
                expect("not_null_orders_order_id", false, 10),
                expect("accepted_values_orders_status__placed__shipped", false, 13),
                expect("source_unique_raw_orders_id", false, 21),
                expect("orders_unit", true, 23),
            ]
        );
    }

    #[test]
    fn broken_yaml_has_no_lenses() {
        assert!(yaml_test_lenses("file:///x.yml", "models: [\n  - :").is_empty());
    }

    #[test]
    fn lens_runs_a_selection() {
        let lenses = yaml_test_lenses("file:///p/models/schema.yml", YAML);
        let first = &lenses[0];
        assert_eq!(first.command.command, "dbt.runSelection");
        assert_eq!(
            first.command.arguments[0],
            serde_json::json!({
                "uri": "file:///p/models/schema.yml",
                "selector": "dbt_utils_expression_is_true_orders_amount_0",
                "kind": "test"
            })
        );
    }
}
