//! dbt's selector methods (dbt-core `graph/selector_methods.py`,
//! `MethodName`): what a `method:` key or a `method:value` prefix names.

/// One selector method. `Config` keeps the dotted key after `config.`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Method {
    Fqn,
    Tag,
    Group,
    Access,
    Source,
    Path,
    File,
    Package,
    Config(String),
    TestName,
    TestType,
    ResourceType,
    Exposure,
    Metric,
    SemanticModel,
    SavedQuery,
    UnitTest,
    Version,
    Wildcard,
    /// A reference to another selectors.yml definition.
    Selector,
    /// Methods that need a previous run's artifacts (`state:`,
    /// `result:`, `source_status:`): reported, never evaluated.
    NeedsState(&'static str),
    Unknown(String),
}

/// Every method a person can write, in the order completion offers them,
/// with a one-line description.
pub const METHODS: &[(&str, &str)] = &[
    ("fqn", "fully qualified name: package, directories, name (the default method)"),
    ("tag", "nodes carrying a tag"),
    ("path", "nodes under a path relative to the project root"),
    ("file", "nodes defined in a file, by file name or stem"),
    ("package", "nodes of a package (`this` is the root project)"),
    ("config", "`config.<key>:<value>` — nodes whose resolved config matches"),
    ("resource_type", "model, seed, snapshot, test, unit_test, source, exposure, metric, semantic_model, analysis"),
    ("source", "sources: `source:src`, `source:src.table`"),
    ("exposure", "exposures by name"),
    ("metric", "metrics by name"),
    ("semantic_model", "semantic models by name"),
    ("saved_query", "saved queries by name"),
    ("test_type", "tests by type: generic, singular, unit (data/schema are aliases)"),
    ("test_name", "generic tests by the test they run (unique, not_null, …)"),
    ("unit_test", "unit tests by name"),
    ("group", "nodes of a governance group"),
    ("access", "models by access: public, protected, private"),
    ("version", "versioned models: latest, prerelease, old, none"),
    ("state", "needs a previous run's manifest (`--state`): not evaluated here"),
    ("result", "needs a previous run's results: not evaluated here"),
    ("source_status", "needs a previous freshness run: not evaluated here"),
    ("selector", "another selector of selectors.yml"),
    ("wildcard", "every node (`*`)"),
];

impl Method {
    /// Parse a method name as written (`tag`, `config.materialized`).
    pub fn parse(name: &str) -> Method {
        if let Some(key) = name.strip_prefix("config.") {
            return Method::Config(key.to_string());
        }
        match name {
            "fqn" => Method::Fqn,
            "tag" => Method::Tag,
            "group" => Method::Group,
            "access" => Method::Access,
            "source" => Method::Source,
            "path" => Method::Path,
            "file" => Method::File,
            "package" => Method::Package,
            "test_name" => Method::TestName,
            "test_type" => Method::TestType,
            "resource_type" => Method::ResourceType,
            "exposure" => Method::Exposure,
            "metric" => Method::Metric,
            "semantic_model" => Method::SemanticModel,
            "saved_query" => Method::SavedQuery,
            "unit_test" => Method::UnitTest,
            "version" => Method::Version,
            "wildcard" => Method::Wildcard,
            "selector" => Method::Selector,
            "state" => Method::NeedsState("state"),
            "result" => Method::NeedsState("result"),
            "source_status" => Method::NeedsState("source_status"),
            other => Method::Unknown(other.to_string()),
        }
    }

    /// dbt's default when a criterion names no method: a path-looking
    /// value is `path`, a `.sql`/`.py`/`.csv` file name is `file`, and
    /// anything else is `fqn` (`SelectionCriteria.default_method`).
    pub fn default_for(value: &str) -> Method {
        if value.contains('/') || value.contains('\\') {
            Method::Path
        } else if [".sql", ".py", ".csv"]
            .iter()
            .any(|ext| value.to_lowercase().ends_with(ext))
        {
            Method::File
        } else {
            Method::Fqn
        }
    }

    /// The name as dbt spells it.
    pub fn name(&self) -> String {
        match self {
            Method::Config(key) => format!("config.{key}"),
            Method::NeedsState(name) => (*name).to_string(),
            Method::Unknown(name) => name.clone(),
            other => {
                let name = match other {
                    Method::Fqn => "fqn",
                    Method::Tag => "tag",
                    Method::Group => "group",
                    Method::Access => "access",
                    Method::Source => "source",
                    Method::Path => "path",
                    Method::File => "file",
                    Method::Package => "package",
                    Method::TestName => "test_name",
                    Method::TestType => "test_type",
                    Method::ResourceType => "resource_type",
                    Method::Exposure => "exposure",
                    Method::Metric => "metric",
                    Method::SemanticModel => "semantic_model",
                    Method::SavedQuery => "saved_query",
                    Method::UnitTest => "unit_test",
                    Method::Version => "version",
                    Method::Wildcard => "wildcard",
                    _ => "selector",
                };
                name.to_string()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn methods_parse_and_print_back() {
        for (name, _) in METHODS {
            if *name == "config" {
                continue;
            }
            let method = Method::parse(name);
            assert!(!matches!(method, Method::Unknown(_)), "{name}");
            assert_eq!(method.name(), *name);
        }
        assert_eq!(
            Method::parse("config.materialized"),
            Method::Config("materialized".into())
        );
        assert_eq!(Method::parse("config.meta.pii").name(), "config.meta.pii");
        assert_eq!(Method::parse("tags"), Method::Unknown("tags".into()));
    }

    #[test]
    fn default_method_follows_dbt() {
        let cases = [
            ("customers", Method::Fqn),
            ("models/staging", Method::Path),
            ("models\\staging", Method::Path),
            ("customers.sql", Method::File),
            ("raw_orders.csv", Method::File),
            ("jaffle_shop.staging", Method::Fqn),
        ];
        for (value, expected) in cases {
            assert_eq!(Method::default_for(value), expected, "{value}");
        }
    }
}
