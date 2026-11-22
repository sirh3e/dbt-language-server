//! Completion in dbt properties YAML (`schema.yml`, `_models.yml`, …),
//! driven by the vendored dbt schema at the cursor's path: the keys of
//! any mapping (`config:`, `columns:` items, test arguments), enum and
//! boolean values, generic test names with their arguments, and a
//! model's output columns under `columns:`. Pure: the shell hands in
//! the columns lineage knows and the generic tests it found on disk.

mod complete;
mod cursor;

pub use complete::{properties_completion, PropertiesInputs};
pub use cursor::{cursor_at, PathStep, Slot, YamlCursor};

use crate::context::DbtContext;

/// A generic test a properties file may name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestDefinition {
    pub name: String,
    /// The package that defines it; None for dbt's own and the project's.
    pub package: Option<String>,
    /// Its macro parameters, `model` and `column_name` included.
    pub params: Vec<String>,
}

impl TestDefinition {
    pub fn new(name: &str, package: Option<&str>, params: &[&str]) -> Self {
        TestDefinition {
            name: name.to_string(),
            package: package.map(str::to_string),
            params: params.iter().map(|p| p.to_string()).collect(),
        }
    }

    /// How a properties file names it: `dbt_utils.at_least_one`.
    pub fn label(&self) -> String {
        match &self.package {
            Some(package) => format!("{package}.{}", self.name),
            None => self.name.clone(),
        }
    }

    /// The parameters a file passes: dbt fills `model` and `column_name`.
    pub fn arguments(&self) -> impl Iterator<Item = &str> {
        self.params
            .iter()
            .map(String::as_str)
            .filter(|p| !matches!(*p, "model" | "column_name"))
    }

    /// True when `written` (as the file spells it) names this test.
    pub fn is_named(&self, written: &str) -> bool {
        written == self.label() || written == self.name
    }
}

/// dbt's four built-in generic tests.
pub fn builtin_tests() -> Vec<TestDefinition> {
    vec![
        TestDefinition::new("not_null", None, &["model", "column_name"]),
        TestDefinition::new("unique", None, &["model", "column_name"]),
        TestDefinition::new(
            "accepted_values",
            None,
            &["model", "column_name", "values", "quote"],
        ),
        TestDefinition::new(
            "relationships",
            None,
            &["model", "column_name", "to", "field"],
        ),
    ]
}

/// Legacy generic tests: macros named `test_<name>` (dbt still calls
/// them for `<name>`). The `{% test %}` blocks come from the shell.
pub fn macro_tests(ctx: &DbtContext) -> Vec<TestDefinition> {
    let own = ctx.project.name.as_str();
    let mut out: Vec<TestDefinition> = ctx
        .macro_detail_map
        .iter()
        .flat_map(|(project, macros)| {
            macros.values().filter_map(move |m| {
                let name = m.name.strip_prefix("test_")?;
                Some(TestDefinition {
                    name: name.to_string(),
                    package: (project != own).then(|| project.clone()),
                    params: m.params.iter().map(|p| p.name.clone()).collect(),
                })
            })
        })
        .collect();
    out.sort_by_key(|t| t.label());
    out
}

/// A dbt properties file, as opposed to the project, package, profile
/// and selector files that share the `.yml` extension.
pub fn is_properties_file(uri: &str) -> bool {
    let name = uri.rsplit(['/', '\\']).next().unwrap_or(uri);
    (name.ends_with(".yml") || name.ends_with(".yaml"))
        && !matches!(
            name,
            "dbt_project.yml"
                | "packages.yml"
                | "dependencies.yml"
                | "profiles.yml"
                | "selectors.yml"
                | "cogitator.yml"
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("file:///p/models/schema.yml", true)]
    #[case("file:///p/models/_models.yaml", true)]
    #[case("file:///p/dbt_project.yml", false)]
    #[case("file:///p/packages.yml", false)]
    #[case("file:///p/models/a.sql", false)]
    fn properties_files(#[case] uri: &str, #[case] expected: bool) {
        assert_eq!(is_properties_file(uri), expected);
    }

    #[test]
    fn a_test_is_named_with_or_without_its_package() {
        let test = TestDefinition::new(
            "at_least_one",
            Some("dbt_utils"),
            &["model", "column_name", "group_by"],
        );
        assert_eq!(test.label(), "dbt_utils.at_least_one");
        assert!(test.is_named("dbt_utils.at_least_one") && test.is_named("at_least_one"));
        assert_eq!(test.arguments().collect::<Vec<_>>(), vec!["group_by"]);
    }
}
