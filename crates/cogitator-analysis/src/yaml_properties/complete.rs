//! The completion items for a cursor in a properties file: schema keys
//! and values at its path, test names and their arguments under
//! `data_tests:`/`tests:`, and the model's output columns as `name:`s.

use cogitator_lsp::{completion_kind, CompletionItem, Position};
use cogitator_schemas::schema_walk::{
    properties_keys_at, properties_values_at, SchemaKey, Step, ValueShape,
};

use super::cursor::{cursor_at, PathStep, Slot};
use super::TestDefinition;

/// What the shell knows beyond the file's text.
#[derive(Debug, Clone, Copy, Default)]
pub struct PropertiesInputs<'a> {
    /// The model whose entry holds the cursor, if any.
    pub model: Option<&'a str>,
    /// Its output columns, as lineage knows them (empty: unknown).
    pub output_columns: &'a [String],
    /// The columns its entry already declares.
    pub declared_columns: &'a [String],
    /// Every generic test the file may name, built-ins included.
    pub tests: &'a [TestDefinition],
}

/// Completion for `text` at `pos` (a properties file).
pub fn properties_completion(
    text: &str,
    pos: Position,
    inputs: &PropertiesInputs<'_>,
) -> Vec<CompletionItem> {
    let Some(cursor) = cursor_at(text, pos) else {
        return Vec::new();
    };
    let path = &cursor.path;
    match &cursor.slot {
        Slot::Key { .. } => key_items(path, inputs),
        Slot::Value { key, .. } => value_items(path, key, inputs),
        Slot::Item { .. } => item_items(path, inputs, ""),
    }
}

fn steps(path: &[PathStep]) -> Vec<Step<'_>> {
    path.iter()
        .map(|s| match s {
            PathStep::Key(k) => Step::Key(k),
            PathStep::Item => Step::Item,
        })
        .collect()
}

fn is_test_list(step: Option<&PathStep>) -> bool {
    matches!(step, Some(PathStep::Key(k)) if k == "data_tests" || k == "tests")
}

/// `[…, tests, -]`: the mapping of one test item.
fn is_test_item(path: &[PathStep]) -> bool {
    path.len() >= 2
        && path[path.len() - 1] == PathStep::Item
        && is_test_list(path.get(path.len() - 2))
}

/// `[…, tests, -, <name>]`: the test named there.
fn test_under(path: &[PathStep]) -> Option<&str> {
    let n = path.len();
    match path {
        [.., PathStep::Key(name)] if n >= 3 && is_test_item(&path[..n - 1]) => Some(name),
        _ => None,
    }
}

/// `[models, -, columns, -]` and `[models, -, versions, -, columns, -]`.
fn is_model_column(path: &[PathStep]) -> bool {
    path.first() == Some(&PathStep::Key("models".into()))
        && path.len() >= 4
        && path[path.len() - 1] == PathStep::Item
        && path[path.len() - 2] == PathStep::Key("columns".into())
}

fn key_items(path: &[PathStep], inputs: &PropertiesInputs<'_>) -> Vec<CompletionItem> {
    if is_test_item(path) {
        return test_items(inputs, "", ":");
    }
    let n = path.len();
    if let [.., PathStep::Key(arguments)] = path {
        if arguments == "arguments" {
            if let Some(test) = test_under(&path[..n - 1]) {
                return argument_items(inputs, test, false);
            }
        }
    }
    let mut out: Vec<CompletionItem> = properties_keys_at(&steps(path))
        .into_iter()
        .map(property_item)
        .collect();
    if let Some(test) = test_under(path) {
        let known: Vec<String> = out.iter().map(|i| i.label.clone()).collect();
        out.extend(
            argument_items(inputs, test, true)
                .into_iter()
                .filter(|i| !known.contains(&i.label)),
        );
    }
    if out.is_empty() {
        // a blank line under a list's key: offer the list's items
        let mut items = path.to_vec();
        items.push(PathStep::Item);
        if !properties_keys_at(&steps(&items)).is_empty() || is_test_list(path.last()) {
            return item_items(path, inputs, "- ");
        }
    }
    out
}

fn value_items(path: &[PathStep], key: &str, inputs: &PropertiesInputs<'_>) -> Vec<CompletionItem> {
    if key == "name" && is_model_column(path) {
        return column_items(inputs, "");
    }
    let mut at = steps(path);
    at.push(Step::Key(key));
    properties_values_at(&at)
        .into_iter()
        .map(value_item)
        .collect()
}

/// Items of the list held by the path's last key; `dash` opens them.
fn item_items(path: &[PathStep], inputs: &PropertiesInputs<'_>, dash: &str) -> Vec<CompletionItem> {
    if is_test_list(path.last()) {
        return test_items(inputs, dash, "");
    }
    let mut item = path.to_vec();
    item.push(PathStep::Item);
    let mut out = Vec::new();
    if is_model_column(&item) {
        out.extend(column_items(inputs, &format!("{dash}name: ")));
    }
    let at = steps(&item);
    out.extend(properties_values_at(&at).into_iter().map(|v| {
        let mut i = value_item(v);
        i.insert_text = format!("{dash}{}", i.insert_text);
        i
    }));
    out.extend(properties_keys_at(&at).into_iter().map(|k| {
        let mut i = property_item(k);
        i.insert_text = format!("{dash}{}", i.insert_text);
        i
    }));
    out
}

/// Test names; a test that takes arguments opens a mapping (`suffix`
/// `:` in a key position, always in an item position).
fn test_items(inputs: &PropertiesInputs<'_>, dash: &str, suffix: &str) -> Vec<CompletionItem> {
    let mut seen = std::collections::HashSet::new();
    inputs
        .tests
        .iter()
        .filter(|t| seen.insert(t.label()))
        .map(|t| {
            let label = t.label();
            let args: Vec<&str> = t.arguments().collect();
            let opens = if args.is_empty() { suffix } else { ":" };
            CompletionItem {
                label: label.clone(),
                detail: match &t.package {
                    Some(package) => format!("generic test ({package})"),
                    None => "generic test".to_string(),
                },
                documentation: if args.is_empty() {
                    String::new()
                } else {
                    format!("arguments: {}", args.join(", "))
                },
                kind: completion_kind::FUNCTION,
                insert_text: format!("{dash}{label}{opens}"),
                sort_text: format!("{}{label}", u8::from(t.package.is_some())),
                data: None,
                ..Default::default()
            }
        })
        .collect()
}

/// The named test's own arguments (`custom` also offers `arguments:`,
/// dbt 1.10's nesting of them).
fn argument_items(inputs: &PropertiesInputs<'_>, test: &str, custom: bool) -> Vec<CompletionItem> {
    let Some(def) = inputs.tests.iter().find(|t| t.is_named(test)) else {
        return Vec::new();
    };
    let mut out: Vec<CompletionItem> = def
        .arguments()
        .map(|arg| CompletionItem {
            label: arg.to_string(),
            detail: format!("argument of {}", def.label()),
            documentation: String::new(),
            kind: completion_kind::VARIABLE,
            insert_text: format!("{arg}: "),
            sort_text: format!("0{arg}"),
            data: None,
            ..Default::default()
        })
        .collect();
    if custom && !out.is_empty() {
        out.push(CompletionItem {
            label: "arguments".to_string(),
            detail: format!("arguments of {} (dbt 1.10+)", def.label()),
            documentation: String::new(),
            kind: completion_kind::VARIABLE,
            insert_text: "arguments:".to_string(),
            sort_text: "1arguments".to_string(),
            data: None,
            ..Default::default()
        });
    }
    out
}

/// The model's output columns, the undeclared ones first.
fn column_items(inputs: &PropertiesInputs<'_>, prefix: &str) -> Vec<CompletionItem> {
    let model = inputs.model.unwrap_or("the model");
    inputs
        .output_columns
        .iter()
        .map(|column| {
            let declared = inputs
                .declared_columns
                .iter()
                .any(|d| d.eq_ignore_ascii_case(column));
            CompletionItem {
                label: column.clone(),
                detail: if declared {
                    format!("column of {model} (declared)")
                } else {
                    format!("column of {model}")
                },
                documentation: String::new(),
                kind: completion_kind::FIELD,
                insert_text: format!("{prefix}{column}"),
                sort_text: format!("{}{column}", u8::from(declared)),
                data: None,
                ..Default::default()
            }
        })
        .collect()
}

fn property_item(key: SchemaKey) -> CompletionItem {
    let insert_text = match key.shape {
        ValueShape::Scalar => format!("{}: ", key.name),
        ValueShape::Mapping | ValueShape::List => format!("{}:", key.name),
    };
    CompletionItem {
        label: key.name.clone(),
        detail: "dbt schema property".to_string(),
        documentation: key.doc,
        kind: completion_kind::VARIABLE,
        insert_text,
        sort_text: key.name,
        data: None,
        ..Default::default()
    }
}

fn value_item(value: String) -> CompletionItem {
    CompletionItem {
        label: value.clone(),
        detail: "dbt schema value".to_string(),
        documentation: String::new(),
        kind: completion_kind::KEYWORD,
        insert_text: value.clone(),
        sort_text: value,
        data: None,
        ..Default::default()
    }
}

#[cfg(test)]
#[path = "complete_tests.rs"]
mod tests;
