//! The node name dbt gives a generic test, so a selector can pick
//! exactly that test. Mirrors dbt-core 1.11's `synthesize_generic_test_names`
//! (parser/generic_test_builders.py): `<test>_<target>_<flattened args>`,
//! the arguments sorted by key (`model` skipped, `column_name` among
//! them), each value flattened (a mapping by its values, a list by its
//! items) into Python's `str()` of each part, and every run of
//! characters outside `[0-9A-Za-z_]` replaced by `_`, joined with `__`.
//! This is the *full* name: once it reaches 64 characters dbt gives the
//! test an alias of its first 30 identifying characters plus an md5,
//! but the node's name and the last element of its FQN stay the full
//! name, which is what a bare `--select <name>` matches (checked
//! against dbt-core 1.11.17, testdata/test_names/golden.json).

use cogitator_schemas::yaml::{as_str, entries, items};
use saphyr::{MarkedYaml, YamlData};

use super::py_yaml::{py_str, Source};

/// Keys of a generic test entry that are configuration, never arguments
/// (dbt's `TestBuilder.CONFIG_ARGS` plus the entry's own keys).
const NON_ARGS: &[&str] = &[
    "severity",
    "tags",
    "enabled",
    "where",
    "limit",
    "warn_if",
    "error_if",
    "fail_calc",
    "store_failures",
    "store_failures_as",
    "meta",
    "database",
    "schema",
    "alias",
    "config",
    "name",
    "description",
    "test_name",
    "arguments",
];

/// The dbt name of a generic test: `test` is the test as written
/// (`not_null`, `dbt_utils.accepted_range`), `target` the node name
/// (`orders`, `orders_v2`, or `<source>_<table>`), `args` the flattened
/// arguments as `(key, parts)`.
pub fn generic_test_name(
    test: &str,
    target: &str,
    is_source: bool,
    args: &[(String, Vec<String>)],
) -> String {
    let (namespace, name) = match test.split_once('.') {
        Some((ns, name)) => (Some(ns), name),
        None => (None, test),
    };
    let mut kind = if is_source {
        format!("source_{name}")
    } else {
        name.to_string()
    };
    if let Some(ns) = namespace {
        kind = format!("{ns}_{kind}");
    }
    let mut sorted: Vec<&(String, Vec<String>)> =
        args.iter().filter(|(k, _)| k != "model").collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    let unique = sorted
        .iter()
        .flat_map(|(_, parts)| parts.iter())
        .map(|part| clean(part))
        .collect::<Vec<_>>()
        .join("__");
    format!("{kind}_{target}_{unique}")
}

/// `re.sub("[^0-9a-zA-Z_]+", "_", arg)`.
fn clean(part: &str) -> String {
    let mut out = String::with_capacity(part.len());
    let mut in_run = false;
    for c in part.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
            in_run = false;
        } else if !in_run {
            out.push('_');
            in_run = true;
        }
    }
    out
}

/// One test entry of a `data_tests:`/`tests:` list, read: the test as
/// written, a `name:` override, and the arguments (flattened, each key
/// once, an `arguments:` entry winning over a top-level one as in dbt).
/// None for an item that names no test.
#[derive(Debug, PartialEq, Eq)]
pub struct TestEntry {
    pub test: String,
    pub custom_name: Option<String>,
    pub args: Vec<(String, Vec<String>)>,
}

impl TestEntry {
    /// The dbt name of this entry on `target`, a column's test with that
    /// column as its `column_name` (unless the arguments name one).
    pub fn name(&self, target: &str, is_source: bool, column: Option<&str>) -> String {
        if let Some(name) = &self.custom_name {
            return name.clone();
        }
        let mut args = self.args.clone();
        if let Some(column) = column {
            if !args.iter().any(|(k, _)| k == "column_name") {
                args.push(("column_name".to_string(), vec![column.to_string()]));
            }
        }
        generic_test_name(&self.test, target, is_source, &args)
    }
}

/// Read one item of a test list; `src` is the document the item is from.
pub fn read_test_entry(item: &MarkedYaml<'_>, src: &Source<'_>) -> Option<TestEntry> {
    if let Some(name) = as_str(item) {
        return Some(TestEntry {
            test: name.to_string(),
            custom_name: None,
            args: Vec::new(),
        });
    }
    let pairs: Vec<_> = entries(item).collect();
    // `- name: x\n  test_name: not_null\n  …` spells the test as a key
    if let Some((_, test)) = pairs.iter().find(|(k, _)| as_str(k) == Some("test_name")) {
        return Some(TestEntry {
            test: as_str(test)?.to_string(),
            custom_name: custom_name(item),
            args: arguments(item, src),
        });
    }
    let (key, body) = pairs.first()?;
    Some(TestEntry {
        test: as_str(key)?.to_string(),
        custom_name: custom_name(body),
        args: arguments(body, src),
    })
}

fn custom_name(body: &MarkedYaml<'_>) -> Option<String> {
    cogitator_schemas::yaml::get(body, "name")
        .and_then(as_str)
        .map(str::to_string)
}

/// The test's arguments: its non-config keys, then (dbt ≥ 1.10, the
/// default `require_generic_test_arguments_property`) the entries of an
/// `arguments:` mapping, which win over a top-level key of the same name.
fn arguments(body: &MarkedYaml<'_>, src: &Source<'_>) -> Vec<(String, Vec<String>)> {
    let own = entries(body).filter(|(k, _)| as_str(k).is_some_and(|k| !NON_ARGS.contains(&k)));
    let nested = cogitator_schemas::yaml::get(body, "arguments")
        .into_iter()
        .flat_map(entries);
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    for (k, v) in own.chain(nested) {
        let Some(k) = as_str(k) else { continue };
        let parts = flatten(v, src);
        match out.iter_mut().find(|(key, _)| key == k) {
            Some(slot) => slot.1 = parts,
            None => out.push((k.to_string(), parts)),
        }
    }
    out
}

/// dbt's flattening of one argument value into its string parts.
fn flatten(value: &MarkedYaml<'_>, src: &Source<'_>) -> Vec<String> {
    match &value.data {
        YamlData::Mapping(_) => entries(value).map(|(_, v)| py_str(v, src)).collect(),
        YamlData::Sequence(_) => items(value).map(|v| py_str(v, src)).collect(),
        _ => vec![py_str(value, src)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use saphyr::LoadableYamlNode;

    fn name_of(yaml: &str, target: &str, column: Option<&str>, source: bool) -> String {
        let docs = MarkedYaml::load_from_str(yaml).unwrap();
        let item = items(&docs[0]).next().unwrap();
        let entry = read_test_entry(item, &Source::new(yaml)).unwrap();
        entry.name(target, source, column)
    }

    #[test]
    fn names_follow_dbt_core() {
        let cases: &[(&str, &str, Option<&str>, bool, &str)] = &[
            ("- not_null", "orders", Some("order_id"), false, "not_null_orders_order_id"),
            ("- unique", "customers", Some("customer_id"), false, "unique_customers_customer_id"),
            (
                "- relationships:\n    to: ref('customers')\n    field: customer_id",
                "orders",
                Some("customer_id"),
                false,
                "relationships_orders_customer_id__customer_id__ref_customers_",
            ),
            (
                "- accepted_values:\n    values: ['placed', 'shipped']\n    config:\n      severity: warn",
                "orders",
                Some("status"),
                false,
                "accepted_values_orders_status__placed__shipped",
            ),
            (
                "- accepted_values:\n    arguments:\n      values: [a, b]\n    where: \"x > 1\"",
                "orders",
                Some("status"),
                false,
                "accepted_values_orders_status__a__b",
            ),
            (
                "- dbt_utils.unique_combination_of_columns:\n    combination_of_columns: [a, b]",
                "orders",
                None,
                false,
                "dbt_utils_unique_combination_of_columns_orders_a__b",
            ),
            ("- not_null", "jaffle_raw_orders", Some("id"), true, "source_not_null_jaffle_raw_orders_id"),
            ("- not_null:\n    name: my_own", "orders", Some("id"), false, "my_own"),
            (
                "- name: renamed\n  test_name: not_null",
                "orders",
                Some("id"),
                false,
                "renamed",
            ),
            (
                "- accepted_values:\n    values: [1, 2.0, true]",
                "orders",
                Some("n"),
                false,
                "accepted_values_orders_n__1__2_0__True",
            ),
        ];
        for (yaml, target, column, source, expected) in cases {
            assert_eq!(
                name_of(yaml, target, *column, *source),
                *expected,
                "for {yaml}"
            );
        }
    }

    #[test]
    fn clean_collapses_runs() {
        assert_eq!(clean("ref('a b')"), "ref_a_b_");
        assert_eq!(clean("a__b"), "a__b");
    }
}
