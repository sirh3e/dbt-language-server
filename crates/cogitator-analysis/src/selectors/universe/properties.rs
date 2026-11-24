//! What a properties YAML says of the nodes a selector reads: each
//! model/seed/snapshot entry's config and versions, each source table's
//! tags and config, the generic tests and the unit tests.

use cogitator_lsp::Range;
use cogitator_schemas::yaml::{as_str, get, items, position_of, to_json};
use saphyr::{LoadableYamlNode, MarkedYaml};
use serde_json::Value;

use super::config::{self, Config};
use crate::features::quality::py_yaml::Source;
use crate::features::quality::test_names::read_test_entry;
use crate::features::quality::test_scope::{applied_tests, Owner};

#[derive(Debug, Default)]
pub struct Properties {
    pub entries: Vec<Entry>,
    pub sources: Vec<SourceTable>,
    pub tests: Vec<GenericTest>,
    pub unit_tests: Vec<UnitTest>,
}

/// A model/seed/snapshot entry.
#[derive(Debug)]
pub struct Entry {
    pub name: String,
    /// `config:` plus the top-level keys dbt treats as config.
    pub config: Config,
    pub latest_version: Option<String>,
    /// (v, defined_in) per version.
    pub versions: Vec<(String, Option<String>)>,
    /// `models`, `seeds` or `snapshots`.
    pub section: &'static str,
    /// A YAML snapshot's `relation:` (dbt 1.9), as written.
    pub relation: Option<String>,
    /// Where its `name:` is.
    pub range: Range,
}

#[derive(Debug)]
pub struct SourceTable {
    pub source: String,
    pub table: String,
    pub config: Config,
    pub range: Range,
}

#[derive(Debug)]
pub struct GenericTest {
    pub name: String,
    pub test_name: String,
    pub owner: Owner,
    /// Models a `relationships` test also depends on (`to: ref('x')`).
    pub also_refs: Vec<String>,
    pub range: Range,
}

#[derive(Debug)]
pub struct UnitTest {
    pub name: String,
    pub model: Option<String>,
    pub range: Range,
}

/// Top-level entry keys dbt folds into the node's config.
const ENTRY_CONFIG_KEYS: &[&str] = &["tags", "meta", "docs", "access", "group"];

pub fn read(text: &str) -> Properties {
    let Ok(docs) = MarkedYaml::load_from_str(text) else {
        return Properties::default();
    };
    let Some(root) = docs.first() else {
        return Properties::default();
    };
    let mut out = Properties::default();
    for key in ["models", "seeds", "snapshots"] {
        for entry in get(root, key).into_iter().flat_map(items) {
            let Some(name_node) = get(entry, "name") else {
                continue;
            };
            let Some(name) = as_str(name_node) else {
                continue;
            };
            let versions = get(entry, "versions")
                .into_iter()
                .flat_map(items)
                .filter_map(|v| {
                    let number = scalar_text(get(v, "v")?)?;
                    let defined_in = get(v, "defined_in").and_then(as_str).map(str::to_string);
                    Some((number, defined_in))
                })
                .collect();
            out.entries.push(Entry {
                name: name.to_string(),
                config: config_of(entry),
                latest_version: get(entry, "latest_version").and_then(scalar_text),
                versions,
                section: key,
                relation: get(entry, "relation").and_then(as_str).map(str::to_string),
                range: Range::zero_width(position_of(name_node)),
            });
        }
    }
    for source in get(root, "sources").into_iter().flat_map(items) {
        let Some(source_name) = get(source, "name").and_then(as_str) else {
            continue;
        };
        let source_config = config_of(source);
        for table in get(source, "tables").into_iter().flat_map(items) {
            let Some(table_name) = get(table, "name") else {
                continue;
            };
            let Some(table_str) = as_str(table_name) else {
                continue;
            };
            let mut table_config = source_config.clone();
            config::layer(&mut table_config, &config_of(table));
            out.sources.push(SourceTable {
                source: source_name.to_string(),
                table: table_str.to_string(),
                config: table_config,
                range: Range::zero_width(position_of(table_name)),
            });
        }
    }
    let src = Source::new(text);
    for applied in applied_tests(root, &src) {
        let Some(entry) = read_test_entry(applied.item, &src) else {
            continue;
        };
        let name = entry.name(&applied.target, applied.is_source, applied.column);
        if out.tests.iter().any(|t| t.name == name) {
            continue;
        }
        let also_refs = entry
            .args
            .iter()
            .filter(|(key, _)| key == "to")
            .flat_map(|(_, values)| values.iter().filter_map(|v| ref_name(v)))
            .collect();
        out.tests.push(GenericTest {
            test_name: entry
                .test
                .rsplit('.')
                .next()
                .unwrap_or(&entry.test)
                .to_string(),
            name,
            owner: applied.owner,
            also_refs,
            range: Range::zero_width(position_of(applied.item)),
        });
    }
    for unit in get(root, "unit_tests").into_iter().flat_map(items) {
        if let Some(name_node) = get(unit, "name") {
            if let Some(name) = as_str(name_node) {
                out.unit_tests.push(UnitTest {
                    name: name.to_string(),
                    model: get(unit, "model").and_then(as_str).map(str::to_string),
                    range: Range::zero_width(position_of(name_node)),
                });
            }
        }
    }
    out
}

fn config_of(entry: &MarkedYaml<'_>) -> Config {
    let mut out = Config::new();
    for key in ENTRY_CONFIG_KEYS {
        if let Some(value) = get(entry, key) {
            out.insert((*key).to_string(), to_json(value));
        }
    }
    if let Some(Value::Object(block)) = get(entry, "config").map(to_json) {
        config::layer(&mut out, &block);
    }
    out
}

fn scalar_text(node: &MarkedYaml<'_>) -> Option<String> {
    match to_json(node) {
        Value::String(s) => Some(s),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// `ref('x')` / `ref("x")` / `ref('pkg', 'x')` → `x`.
pub(super) fn ref_name(text: &str) -> Option<String> {
    let inner = text.trim().strip_prefix("ref(")?.strip_suffix(')')?;
    let last = inner.rsplit(',').next()?.trim();
    let name = last.trim_matches(|c| c == '\'' || c == '"');
    (!name.is_empty()).then(|| name.to_string())
}

/// `source('s', 't')` → (`s`, `t`).
pub(super) fn source_name(text: &str) -> Option<(String, String)> {
    let inner = text.trim().strip_prefix("source(")?.strip_suffix(')')?;
    let mut parts = inner
        .split(',')
        .map(|p| p.trim().trim_matches(|c| c == '\'' || c == '"').to_string());
    let (source, table) = (parts.next()?, parts.next()?);
    (!source.is_empty() && !table.is_empty() && parts.next().is_none()).then_some((source, table))
}

#[cfg(test)]
mod tests {
    use super::*;

    const YAML: &str = "\
version: 2
models:
  - name: orders
    tags: [nightly]
    config:
      materialized: table
      tags: pii
    columns:
      - name: customer_id
        data_tests:
          - not_null
          - relationships:
              to: ref('customers')
              field: customer_id
  - name: customer_orders
    latest_version: 2
    versions:
      - v: 1
      - v: 2
        defined_in: co_two
sources:
  - name: raw
    tags: [landing]
    tables:
      - name: payments
        config:
          tags: [money]
unit_tests:
  - name: orders_are_counted
    model: orders
";

    #[test]
    fn a_properties_file_is_read() {
        let p = read(YAML);
        let orders = &p.entries[0];
        assert_eq!(orders.config["materialized"], "table");
        assert_eq!(orders.config["tags"], serde_json::json!(["nightly", "pii"]));
        let co = &p.entries[1];
        assert_eq!(co.latest_version.as_deref(), Some("2"));
        assert_eq!(
            co.versions,
            vec![("1".into(), None), ("2".into(), Some("co_two".into()))]
        );
        assert_eq!(
            p.sources[0].config["tags"],
            serde_json::json!(["landing", "money"])
        );
        let names: Vec<_> = p
            .tests
            .iter()
            .map(|t| (t.name.as_str(), t.test_name.as_str()))
            .collect();
        assert_eq!(
            names,
            vec![
                ("not_null_orders_customer_id", "not_null"),
                (
                    "relationships_orders_customer_id__customer_id__ref_customers_",
                    "relationships"
                ),
            ]
        );
        assert_eq!(p.tests[1].also_refs, vec!["customers".to_string()]);
        assert_eq!(p.unit_tests[0].model.as_deref(), Some("orders"));
    }

    #[test]
    fn ref_names() {
        for (text, expected) in [
            ("ref('a')", Some("a")),
            ("ref(\"pkg\", \"b\")", Some("b")),
            ("source('a','b')", None),
        ] {
            assert_eq!(ref_name(text).as_deref(), expected, "{text}");
        }
    }
}
