//! `functions:` entries of a properties file: dbt's user-defined
//! functions (dbt 1.11+, <https://docs.getdbt.com/docs/build/udfs>).
//!
//! Read from the marked tree directly, like the properties side-tables:
//! the server needs names, positions and the signature only, and a
//! function's config varies by warehouse, so nothing here is strict. An
//! entry without a name is dropped; everything else is optional.

use std::path::Path;

use cogitator_error::Result;
use cogitator_lsp::Position;

use crate::yaml::{self, MarkedYaml};

/// One `functions:` entry.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FunctionEntry {
    pub name: String,
    /// Position of the entry's `name:` value.
    pub name_pos: Position,
    pub description: String,
    pub arguments: Vec<FunctionArgument>,
    /// `returns:` — required by dbt, `None` when the file leaves it out.
    pub returns: Option<FunctionReturns>,
    /// `config.type`: `scalar` (dbt's default) or `aggregate`.
    pub function_type: Option<String>,
    /// `config.volatility`: `deterministic`, `stable` or `non-deterministic`.
    pub volatility: Option<String>,
    /// `overloads[].defined_in` (dbt 1.12+): the body files, by name, of
    /// the function's other signatures.
    pub overloads: Vec<String>,
}

/// One entry of `arguments:`.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FunctionArgument {
    pub name: String,
    pub data_type: String,
    pub description: String,
    /// `default_value:` as written (a string, number or boolean).
    pub default_value: Option<String>,
}

/// `returns:` of an entry.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FunctionReturns {
    pub data_type: String,
    pub description: String,
}

/// Every `functions:` entry of one properties YAML text; a file without
/// the key gives none.
pub fn parse_functions(text: &str, path: &Path) -> Result<Vec<FunctionEntry>> {
    let marked = yaml::load_marked(text, path)?;
    Ok(yaml::get(&marked, "functions")
        .into_iter()
        .flat_map(yaml::items)
        .filter_map(function_entry)
        .collect())
}

fn function_entry(node: &MarkedYaml<'_>) -> Option<FunctionEntry> {
    let name_node = yaml::get(node, "name")?;
    let name = yaml::as_str(name_node)?.trim().to_string();
    if name.is_empty() {
        return None;
    }
    let config = |key: &str| yaml::get(node, "config").and_then(|c| text(c, key));
    Some(FunctionEntry {
        name,
        name_pos: yaml::position_of(name_node),
        description: text(node, "description").unwrap_or_default(),
        arguments: yaml::get(node, "arguments")
            .into_iter()
            .flat_map(yaml::items)
            .filter_map(argument)
            .collect(),
        returns: yaml::get(node, "returns").map(|r| FunctionReturns {
            data_type: text(r, "data_type").unwrap_or_default(),
            description: text(r, "description").unwrap_or_default(),
        }),
        function_type: config("type"),
        volatility: config("volatility"),
        overloads: yaml::get(node, "overloads")
            .into_iter()
            .flat_map(yaml::items)
            .filter_map(|o| text(o, "defined_in"))
            .collect(),
    })
}

fn argument(node: &MarkedYaml<'_>) -> Option<FunctionArgument> {
    Some(FunctionArgument {
        name: text(node, "name")?,
        data_type: text(node, "data_type").unwrap_or_default(),
        description: text(node, "description").unwrap_or_default(),
        default_value: yaml::get(node, "default_value").map(scalar_text),
    })
}

/// A scalar child as text: strings as they are, numbers and booleans as
/// YAML wrote them.
fn text(node: &MarkedYaml<'_>, key: &str) -> Option<String> {
    yaml::get(node, key)
        .map(scalar_text)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn scalar_text(node: &MarkedYaml<'_>) -> String {
    match yaml::to_json(node) {
        serde_json::Value::String(s) => s,
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    const YAML: &str = "\
version: 2
functions:
  - name: is_positive_int
    description: Whether a string is a positive integer
    config:
      type: scalar
      volatility: deterministic
    arguments:
      - name: a_string
        data_type: string
        description: The text to check
      - name: strict
        data_type: boolean
        default_value: true
    returns:
      data_type: boolean
    overloads:
      - defined_in: is_positive_int_numeric
  - name: ''
  - description: no name
";

    fn parsed() -> Vec<FunctionEntry> {
        parse_functions(YAML, Path::new("functions/schema.yml")).unwrap()
    }

    #[test]
    fn an_entry_carries_its_signature() {
        let f = &parsed()[0];
        assert_eq!(f.name, "is_positive_int");
        assert_eq!(
            f.name_pos,
            Position {
                line: 2,
                character: 10
            }
        );
        assert_eq!(f.description, "Whether a string is a positive integer");
        assert_eq!(f.function_type.as_deref(), Some("scalar"));
        assert_eq!(f.volatility.as_deref(), Some("deterministic"));
        assert_eq!(f.overloads, vec!["is_positive_int_numeric".to_string()]);
        assert_eq!(
            f.arguments,
            vec![
                FunctionArgument {
                    name: "a_string".into(),
                    data_type: "string".into(),
                    description: "The text to check".into(),
                    default_value: None,
                },
                FunctionArgument {
                    name: "strict".into(),
                    data_type: "boolean".into(),
                    description: String::new(),
                    default_value: Some("true".into()),
                },
            ]
        );
        assert_eq!(
            f.returns,
            Some(FunctionReturns {
                data_type: "boolean".into(),
                description: String::new(),
            })
        );
    }

    #[test]
    fn entries_without_a_name_are_dropped() {
        assert_eq!(parsed().len(), 1);
    }

    #[test]
    fn a_file_without_functions_has_none() {
        let models = "version: 2\nmodels:\n  - name: customers\n";
        assert!(parse_functions(models, Path::new("m.yml"))
            .unwrap()
            .is_empty());
    }
}
