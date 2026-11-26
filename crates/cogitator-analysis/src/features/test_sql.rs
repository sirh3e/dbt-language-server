//! dbt.testSql: the SQL every test of a model runs, rendered without
//! `dbt compile`. A data test of the properties YAML is rendered by
//! calling its macro (the project's or a package's, else dbt's own
//! `not_null`, `unique`, `accepted_values` and `relationships`) with its
//! arguments; a singular test (a `.sql` file under the test paths that
//! refs the model) is rendered as it is. Each comes twice: the test's
//! own SQL, the rows that fail, and the query dbt executes around it
//! (`failures`, `should_warn`, `should_error`, with `where:`, `limit:`,
//! `warn_if:`, `error_if:` and `fail_calc:` applied). A unit test is
//! listed, with nothing to render. Pure: the shell assembles the
//! [`PreviewInputs`] and the test files' texts.

use std::sync::LazyLock;

use cogitator_lsp::Range;
use cogitator_schemas::yaml::{as_str, entries, get, items, position_of, to_json};
use minijinja::value::Value;
use saphyr::{LoadableYamlNode, MarkedYaml};
use serde::Serialize;
use serde_json::{Map, Value as Json};

use super::jinja_preview::{environment, render_error, PreviewInputs};
use super::tests_nav::{model_entry, resolve_model_name};
use super::{file_uri, model_name_from_uri, DbtContext};

/// The dbt.testSql answer.
#[derive(Debug, Serialize)]
pub struct TestSql {
    pub model: String,
    /// The relation the tests read (`{{ ref(model) }}`).
    pub relation: String,
    pub tests: Vec<RenderedTest>,
    /// Every test with its query, as one SQL text to show.
    pub text: String,
}

/// One test of the model and what it runs.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderedTest {
    /// What a reader calls it: `not_null on order_id`, a file's name.
    pub label: String,
    /// The test's name as the YAML writes it (`dbt_utils.expression_is_true`).
    pub name: String,
    pub column: Option<String>,
    pub kind: TestKind,
    /// The macro that rendered it, or `dbt (built-in)`.
    pub defined_by: Option<String>,
    pub severity: String,
    /// Where the test is declared.
    pub uri: String,
    pub range: Range,
    /// The rows that fail the test.
    pub sql: Option<String>,
    /// What dbt executes: the failures counted around [`Self::sql`].
    pub query: Option<String>,
    /// Why it could not be rendered.
    pub error: Option<String>,
    /// A remark about the test (a unit test has nothing to render).
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TestKind {
    Generic,
    Singular,
    Unit,
}

/// A singular test file the shell read: its path and text.
pub struct SingularFile {
    pub path: std::path::PathBuf,
    pub text: String,
}

/// One data test of the properties YAML, with what it is called with.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TestSpec {
    pub name: String,
    pub column: Option<String>,
    pub args: Map<String, Json>,
    pub config: Map<String, Json>,
    pub range: Range,
}

/// The tests of the model `uri` is, rendered. `None` when the file is
/// not a model of the project.
pub fn test_sql(
    ctx: &DbtContext,
    uri: &str,
    inputs: &PreviewInputs,
    singular: &[SingularFile],
) -> Option<TestSql> {
    let model = resolve_model_name(ctx, &model_name_from_uri(uri)?)?;
    let details = ctx.model_detail_map.get(&model)?;
    let relation = inputs
        .model_relations
        .get(&model)
        .cloned()
        .unwrap_or_else(|| model.clone());
    let renderer = TestRenderer::new(inputs, ctx.dialect.as_str());
    let mut tests = Vec::new();
    if let Some(schema_path) = &details.schema_uri {
        if let Ok(text) = std::fs::read_to_string(schema_path) {
            let schema_uri = file_uri(schema_path);
            tests.extend(yaml_tests(&text, &model, &schema_uri, &renderer));
        }
    }
    tests.extend(singular.iter().map(|file| renderer.singular(&model, file)));
    let mut answer = TestSql {
        model,
        relation,
        tests,
        text: String::new(),
    };
    answer.text = shown_text(&answer);
    Some(answer)
}

/// The data and unit tests a properties file declares for `model`.
pub(crate) fn yaml_tests(
    text: &str,
    model: &str,
    schema_uri: &str,
    renderer: &TestRenderer,
) -> Vec<RenderedTest> {
    let Ok(docs) = MarkedYaml::load_from_str(text) else {
        return Vec::new();
    };
    let Some(root) = docs.first() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(entry) = model_entry(root, model) {
        out.extend(
            specs_of(entry)
                .into_iter()
                .map(|spec| renderer.generic(model, &spec, schema_uri)),
        );
    }
    out.extend(
        super::tests_nav::collect_unit_tests(root, model, schema_uri)
            .into_iter()
            .map(|unit| RenderedTest {
                label: format!("unit test {}", unit.name),
                name: unit.name,
                column: None,
                kind: TestKind::Unit,
                defined_by: None,
                severity: "error".to_string(),
                uri: unit.uri,
                range: unit.range,
                sql: None,
                query: None,
                error: None,
                note: Some(
                    "a unit test runs the model on the rows it gives and compares the result; \
                 it has no SQL of its own"
                        .to_string(),
                ),
            }),
    );
    out
}

/// Every data test of a model entry: model-level, per column, per version.
pub(crate) fn specs_of(entry: &MarkedYaml<'_>) -> Vec<TestSpec> {
    let mut out = Vec::new();
    node_specs(entry, None, &mut out);
    for column in get(entry, "columns").into_iter().flat_map(items) {
        if let Some(name) = get(column, "name").and_then(as_str) {
            node_specs(column, Some(name), &mut out);
        }
    }
    for version in get(entry, "versions").into_iter().flat_map(items) {
        node_specs(version, None, &mut out);
        for column in get(version, "columns").into_iter().flat_map(items) {
            if let Some(name) = get(column, "name").and_then(as_str) {
                node_specs(column, Some(name), &mut out);
            }
        }
    }
    out
}

fn node_specs(node: &MarkedYaml<'_>, column: Option<&str>, out: &mut Vec<TestSpec>) {
    for key in ["data_tests", "tests"] {
        for item in get(node, key).into_iter().flat_map(items) {
            let range = Range::zero_width(position_of(item));
            if let Some(name) = as_str(item) {
                out.push(TestSpec {
                    name: name.to_string(),
                    column: column.map(str::to_string),
                    args: Map::new(),
                    config: Map::new(),
                    range,
                });
                continue;
            }
            // `- accepted_values: {values: [...], config: {...}}`
            let Some((key, body)) = entries(item).next() else {
                continue;
            };
            let Some(name) = as_str(key) else {
                continue;
            };
            let body = match to_json(body) {
                Json::Object(map) => map,
                _ => Map::new(),
            };
            let (args, config) = split_body(body);
            out.push(TestSpec {
                name: name.to_string(),
                column: column.map(str::to_string),
                args,
                config,
                range,
            });
        }
    }
}

/// A configured test's body as its arguments and its config. dbt 1.10
/// writes the arguments under `arguments:`; older projects beside the
/// config keys, which dbt also reads from the top.
fn split_body(mut body: Map<String, Json>) -> (Map<String, Json>, Map<String, Json>) {
    const CONFIG_KEYS: &[&str] = &[
        "severity",
        "where",
        "limit",
        "warn_if",
        "error_if",
        "fail_calc",
        "store_failures",
        "tags",
        "enabled",
        "meta",
    ];
    let mut config = match body.remove("config") {
        Some(Json::Object(map)) => map,
        _ => Map::new(),
    };
    body.remove("name");
    body.remove("description");
    for key in CONFIG_KEYS {
        if let Some(value) = body.remove(*key) {
            config.entry(key.to_string()).or_insert(value);
        }
    }
    let args = match body.remove("arguments") {
        Some(Json::Object(map)) => map,
        _ => body,
    };
    (args, config)
}

/// dbt's own generic tests, as dbt-core's `default__test_*` macros
/// render them.
const BUILTIN_TESTS: &[(&str, &str)] = &[
    (
        "not_null",
        "{% macro default__test_not_null(model, column_name) %}\
select {{ column_name }}\nfrom {{ model }}\nwhere {{ column_name }} is null\n\
{% endmacro %}",
    ),
    (
        "unique",
        "{% macro default__test_unique(model, column_name) %}\
select\n    {{ column_name }} as unique_field,\n    count(*) as n_records\n\n\
from {{ model }}\nwhere {{ column_name }} is not null\n\
group by {{ column_name }}\nhaving count(*) > 1\n\
{% endmacro %}",
    ),
    (
        "accepted_values",
        "{% macro default__test_accepted_values(model, column_name, values, quote=true) %}\
with all_values as (\n\n    select\n        {{ column_name }} as value_field,\n        \
count(*) as n_records\n\n    from {{ model }}\n    group by {{ column_name }}\n\n)\n\n\
select *\nfrom all_values\nwhere value_field not in (\n    \
{%- for value in values %}{% if quote %}'{{ value }}'{% else %}{{ value }}{% endif %}\
{% if not loop.last %}, {% endif %}{% endfor %}\n)\n\
{% endmacro %}",
    ),
    (
        "relationships",
        "{% macro default__test_relationships(model, column_name, to, field) %}\
with child as (\n    select {{ column_name }} as from_field\n    from {{ model }}\n    \
where {{ column_name }} is not null\n),\n\nparent as (\n    select {{ field }} as to_field\n    \
from {{ to }}\n)\n\nselect\n    from_field\n\nfrom child\nleft join parent\n    \
on child.from_field = parent.to_field\n\nwhere parent.to_field is null\n\
{% endmacro %}",
    ),
];

/// Render budget (minijinja fuel) per test, as for the syntax check.
const RENDER_FUEL: u64 = 2_000_000;

static MACRO_NAME_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"\{%-?\s*macro\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(").expect("valid regex")
});

/// Renders tests over one project state.
pub(crate) struct TestRenderer<'a> {
    inputs: &'a PreviewInputs,
    adapter: String,
    /// The project's and the packages' macro files that parse.
    macros: String,
    names: std::collections::HashSet<String>,
}

impl<'a> TestRenderer<'a> {
    pub(crate) fn new(inputs: &'a PreviewInputs, adapter: &str) -> Self {
        let scratch = minijinja::Environment::new();
        let mut macros = String::new();
        for source in &inputs.macro_sources {
            let Some(text) = super::rename_local::jinja::parseable(source) else {
                continue;
            };
            if scratch.template_from_str(&text).is_err() {
                continue;
            }
            macros.push_str(&text);
            macros.push('\n');
        }
        let names = MACRO_NAME_RE
            .captures_iter(&macros)
            .map(|c| c[1].to_string())
            .collect();
        TestRenderer {
            inputs,
            adapter: adapter.to_string(),
            macros,
            names,
        }
    }

    /// The macro that renders the test `name` and its source, the way
    /// `adapter.dispatch` would pick it: the adapter's, the default, the
    /// test itself; else dbt's built-in one.
    fn macro_for(&self, name: &str) -> Option<(String, Option<&'static str>)> {
        let candidates = [
            format!("{}__test_{name}", self.adapter),
            format!("default__test_{name}"),
            format!("test_{name}"),
            name.to_string(),
        ];
        if let Some(found) = candidates.into_iter().find(|c| self.names.contains(c)) {
            return Some((found, None));
        }
        BUILTIN_TESTS
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(n, source)| (format!("default__test_{n}"), Some(*source)))
    }

    pub(crate) fn generic(&self, model: &str, spec: &TestSpec, uri: &str) -> RenderedTest {
        // `dbt_utils.expression_is_true`: the namespace is the package
        let bare = spec.name.rsplit('.').next().unwrap_or(&spec.name);
        let label = match &spec.column {
            Some(column) => format!("{} on {column}", spec.name),
            None => spec.name.clone(),
        };
        let mut test = RenderedTest {
            label,
            name: spec.name.clone(),
            column: spec.column.clone(),
            kind: TestKind::Generic,
            defined_by: None,
            severity: severity_of(&spec.config),
            uri: uri.to_string(),
            range: spec.range,
            sql: None,
            query: None,
            error: None,
            note: None,
        };
        let Some((macro_name, builtin)) = self.macro_for(bare) else {
            test.error = Some(format!(
                "no macro defines the test '{}' (looked for default__test_{bare}, test_{bare} \
                 and a {{% test {bare} %}} block)",
                spec.name
            ));
            return test;
        };
        test.defined_by = Some(match builtin {
            Some(_) => "dbt (built-in)".to_string(),
            None => macro_name.clone(),
        });
        let mut call = format!(
            "{{{{ {macro_name}(model={}",
            model_expression(model, &spec.config)
        );
        if let Some(column) = &spec.column {
            call.push_str(&format!(", column_name={}", jinja_string(column)));
        }
        for (key, value) in &spec.args {
            call.push_str(&format!(", {key}={}", jinja_argument(value)));
        }
        call.push_str(") }}");
        let preamble = builtin.unwrap_or("");
        match self.render(&format!("{preamble}\n{call}")) {
            Ok(sql) => {
                test.query = Some(executed_query(&sql, &spec.config));
                test.sql = Some(sql);
            }
            Err(error) => test.error = Some(error),
        }
        test
    }

    pub(crate) fn singular(&self, model: &str, file: &SingularFile) -> RenderedTest {
        let name = file
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let config = test_file_config(&file.text);
        let mut test = RenderedTest {
            label: format!("singular test {name}"),
            name,
            column: None,
            kind: TestKind::Singular,
            defined_by: None,
            severity: severity_of(&config),
            uri: file_uri(&file.path),
            range: Range::default(),
            sql: None,
            query: None,
            error: None,
            note: Some(format!("a test file that refs '{model}'")),
        };
        match self.render(&file.text) {
            Ok(sql) => {
                test.query = Some(executed_query(&sql, &config));
                test.sql = Some(sql);
            }
            Err(error) => test.error = Some(error),
        }
        test
    }

    /// `body` rendered after the macros, its leading white space trimmed.
    fn render(&self, body: &str) -> Result<String, String> {
        let mut env = environment(self.inputs);
        env.set_fuel(Some(RENDER_FUEL));
        env.add_global("execute", Value::from(true));
        let mut template = String::with_capacity(self.macros.len() + body.len());
        template.push_str(&self.macros);
        template.push('\n');
        template.push_str(body);
        env.render_str(&template, minijinja::context! {})
            .map(|sql| tidy(&sql))
            .map_err(|err| render_error(err).to_diagnostic())
    }
}

/// The relation a test reads: the model, or with `where:` the model's
/// rows that pass it, as dbt wraps it.
fn model_expression(model: &str, config: &Map<String, Json>) -> String {
    let rel = format!("ref({})", jinja_string(model));
    match config.get("where").and_then(Json::as_str) {
        Some(condition) => format!(
            "'(select * from ' ~ {rel} ~ ' where ' ~ {} ~ ') dbt_subquery'",
            jinja_string(condition)
        ),
        None => rel,
    }
}

/// A YAML argument as a Jinja expression. dbt renders a string argument
/// as Jinja, so `to: ref('customers')` is the relation, not the text.
fn jinja_argument(value: &Json) -> String {
    match value {
        Json::String(s) => {
            let t = s.trim();
            let call = ["ref(", "source(", "var(", "env_var("]
                .iter()
                .any(|p| t.starts_with(p))
                && t.ends_with(')');
            let rendered = t.starts_with("{{") && t.ends_with("}}");
            if call {
                t.to_string()
            } else if rendered {
                t[2..t.len() - 2].trim().to_string()
            } else {
                jinja_string(s)
            }
        }
        Json::Null => "none".to_string(),
        Json::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(jinja_literal)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        other => jinja_literal(other),
    }
}

/// A JSON value as a Jinja literal (no rendering of strings).
fn jinja_literal(value: &Json) -> String {
    match value {
        Json::String(s) => jinja_string(s),
        Json::Null => "none".to_string(),
        Json::Bool(b) => b.to_string(),
        Json::Number(n) => n.to_string(),
        Json::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(jinja_literal)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Json::Object(map) => format!(
            "{{{}}}",
            map.iter()
                .map(|(k, v)| format!("{}: {}", jinja_string(k), jinja_literal(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn jinja_string(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| format!("'{s}'"))
}

fn severity_of(config: &Map<String, Json>) -> String {
    config
        .get("severity")
        .and_then(Json::as_str)
        .map(str::to_ascii_lowercase)
        .unwrap_or_else(|| "error".to_string())
}

/// A singular test's own `{{ config(...) }}` keys.
fn test_file_config(text: &str) -> Map<String, Json> {
    crate::util::sql_config(text).into_iter().collect()
}

/// The query dbt runs around a test's SQL (dbt-core's
/// `default__get_test_sql`).
pub(crate) fn executed_query(sql: &str, config: &Map<String, Json>) -> String {
    let text = |key: &str, default: &str| {
        config
            .get(key)
            .and_then(Json::as_str)
            .unwrap_or(default)
            .to_string()
    };
    let fail_calc = text("fail_calc", "count(*)");
    let warn_if = text("warn_if", "!= 0");
    let error_if = text("error_if", "!= 0");
    let limit = config
        .get("limit")
        .and_then(Json::as_u64)
        .map(|n| format!("\n    limit {n}"))
        .unwrap_or_default();
    let body = sql
        .trim_end()
        .lines()
        .map(|l| {
            if l.is_empty() {
                String::new()
            } else {
                format!("    {l}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "select\n    {fail_calc} as failures,\n    {fail_calc} {warn_if} as should_warn,\n    \
         {fail_calc} {error_if} as should_error\nfrom (\n{body}{limit}\n) dbt_internal_test\n"
    )
}

/// The rendered text without the macros' blank preamble and with at
/// most one blank line in a row.
fn tidy(sql: &str) -> String {
    let mut out = String::new();
    let mut blank = 0;
    for line in sql.trim().lines() {
        if line.trim().is_empty() {
            blank += 1;
            if blank > 1 {
                continue;
            }
        } else {
            blank = 0;
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

/// The text the code action opens: every test with its query.
pub fn shown_text(answer: &TestSql) -> String {
    let mut out = format!(
        "-- The tests of {} ({}), rendered as dbt runs them.\n",
        answer.model, answer.relation
    );
    if answer.tests.is_empty() {
        out.push_str("-- The model has no tests.\n");
    }
    for test in &answer.tests {
        out.push_str(&format!(
            "\n-- ── {} · severity {}{}\n",
            test.label,
            test.severity,
            test.defined_by
                .as_deref()
                .map(|d| format!(" · {d}"))
                .unwrap_or_default()
        ));
        if let Some(note) = &test.note {
            out.push_str(&format!("-- {note}\n"));
        }
        match (&test.query, &test.error) {
            (Some(query), _) => out.push_str(query),
            (None, Some(error)) => out.push_str(&format!("-- could not render: {error}\n")),
            _ => {}
        }
    }
    out
}

/// "Show the SQL of N tests" on a model that has tests: the command
/// opens them as a document (`show: true`) in every client.
pub fn test_sql_action(ctx: &DbtContext, uri: &str) -> Option<cogitator_lsp::CodeAction> {
    let model = resolve_model_name(ctx, &model_name_from_uri(uri)?)?;
    let declared = super::model_tests(ctx, uri)
        .map(|(_, tests)| tests.iter().filter(|t| !t.unit).count())
        .unwrap_or(0);
    let singular = ctx
        .graph
        .downstream_of(&model)
        .filter(|n| n.kind == crate::graph::NodeKind::Test)
        .count();
    let count = declared + singular;
    (count > 0).then(|| {
        let title = format!(
            "Show the SQL of {count} test{}",
            if count == 1 { "" } else { "s" }
        );
        cogitator_lsp::CodeAction::command_only(
            title.clone(),
            cogitator_lsp::Command {
                title,
                command: "dbt.testSql".to_string(),
                arguments: vec![serde_json::json!({"uri": uri, "show": true})],
            },
        )
    })
}

#[cfg(test)]
#[path = "test_sql_tests.rs"]
mod tests;
