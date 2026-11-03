//! dbt.expandJinja (05 §9): render the model's Jinja with minijinja —
//! macros from the project's macro files, ref()/source() answering
//! resolved relations, var() from the project variables, and hand
//! stubs for config()/is_incremental()/target. A poor man's compile
//! without dbt, for "what does this loop emit".

use std::collections::HashMap;

use cogitator_error::guidance::Guidance;
use minijinja::value::Value;
use minijinja::{Environment, Error, ErrorKind};
use serde::Serialize;

/// The dbt.expandJinja answer.
#[derive(Debug, Serialize)]
pub struct ExpandedSql {
    pub sql: String,
}

/// Everything the renderer needs, assembled by the shell.
pub struct PreviewInputs {
    /// Whole macro files, concatenated ahead of the model body.
    pub macro_sources: Vec<String>,
    /// model name → relation string ("jaffle.main.customers").
    pub model_relations: HashMap<String, String>,
    /// (source, table) → relation string.
    pub source_relations: HashMap<(String, String), String>,
    /// project var name → value.
    pub vars: HashMap<String, serde_json::Value>,
    pub target_name: String,
}

pub fn expand_jinja(body: &str, inputs: &PreviewInputs) -> Result<ExpandedSql, Guidance> {
    let mut env = Environment::new();
    let models = inputs.model_relations.clone();
    let sources = inputs.source_relations.clone();
    let vars = inputs.vars.clone();

    env.add_function("ref", move |args: &[Value]| -> Result<Value, Error> {
        let names: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        let model = names.last().cloned().unwrap_or_default();
        Ok(Value::from(models.get(&model).cloned().unwrap_or(model)))
    });
    env.add_function(
        "source",
        move |source: String, table: String| -> Result<Value, Error> {
            Ok(Value::from(
                sources
                    .get(&(source.clone(), table.clone()))
                    .cloned()
                    .unwrap_or(format!("{source}.{table}")),
            ))
        },
    );
    env.add_function(
        "var",
        move |name: String, default: Option<Value>| -> Result<Value, Error> {
            match vars.get(&name) {
                Some(value) => Ok(json_to_value(value)),
                None => default.ok_or_else(|| {
                    Error::new(
                        ErrorKind::UndefinedError,
                        format!("var('{name}') has no project value and no default"),
                    )
                }),
            }
        },
    );
    env.add_function("config", |_kwargs: minijinja::value::Kwargs| {
        Ok::<Value, Error>(Value::from(""))
    });
    env.add_function("is_incremental", || Ok::<Value, Error>(Value::from(false)));
    env.add_global(
        "target",
        Value::from(std::collections::BTreeMap::from([(
            "name".to_string(),
            Value::from(inputs.target_name.clone()),
        )])),
    );

    // macros first, body last: one template, dbt-ish scoping
    let mut template = String::new();
    for source in &inputs.macro_sources {
        template.push_str(source);
        template.push('\n');
    }
    template.push_str(body);

    env.add_template("model", &template).map_err(render_error)?;
    let rendered = env
        .get_template("model")
        .and_then(|t| t.render(minijinja::context! {}))
        .map_err(render_error)?;

    // the macro preamble renders to whitespace; trim the noise away
    Ok(ExpandedSql {
        sql: rendered.trim_start_matches(['\n', ' ', '\t']).to_string(),
    })
}

/// serde_json → minijinja without the serde bridge feature.
fn json_to_value(value: &serde_json::Value) -> Value {
    match value {
        serde_json::Value::Null => Value::from(()),
        serde_json::Value::Bool(b) => Value::from(*b),
        serde_json::Value::Number(n) => n
            .as_i64()
            .map(Value::from)
            .or_else(|| n.as_f64().map(Value::from))
            .unwrap_or_else(|| Value::from(n.to_string())),
        serde_json::Value::String(text) => Value::from(text.clone()),
        serde_json::Value::Array(items) => {
            Value::from(items.iter().map(json_to_value).collect::<Vec<_>>())
        }
        serde_json::Value::Object(map) => Value::from(
            map.iter()
                .map(|(k, v)| (k.clone(), json_to_value(v)))
                .collect::<std::collections::BTreeMap<_, _>>(),
        ),
    }
}

fn render_error(err: Error) -> Guidance {
    let mut guidance = Guidance::new("Jinja preview could not render.").because(err.to_string());
    if let Some(line) = err.line() {
        guidance = guidance.at(format!("template line {line}"));
    }
    guidance.fix(
        "the preview stubs a subset (ref/source/var/config/is_incremental/target); \
         adapter calls and incremental blocks need a real dbt compile.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs() -> PreviewInputs {
        PreviewInputs {
            macro_sources: vec!["{% macro double(x) %}{{ x * 2 }}{% endmacro %}".to_string()],
            model_relations: HashMap::from([(
                "customers".to_string(),
                "jaffle.main.customers".to_string(),
            )]),
            source_relations: HashMap::from([(
                ("stripe".to_string(), "payments".to_string()),
                "raw.stripe.payments".to_string(),
            )]),
            vars: HashMap::from([("n".to_string(), serde_json::json!(3))]),
            target_name: "dev".to_string(),
        }
    }

    #[test]
    fn renders_loops_refs_vars_and_macros() {
        let body = "{{ config(materialized='table') }}\n\
                    select {% for i in range(var('n')) %}{{ double(i) }}{{ ',' if not loop.last }}{% endfor %}\n\
                    from {{ ref('customers') }} join {{ source('stripe', 'payments') }} using (id)\n\
                    where target_is = '{{ target.name }}' and incr = {{ is_incremental() }}";
        let sql = expand_jinja(body, &inputs()).expect("renders").sql;
        assert!(sql.contains("select 0,2,4"), "{sql}");
        assert!(sql.contains("from jaffle.main.customers"), "{sql}");
        assert!(sql.contains("raw.stripe.payments"), "{sql}");
        assert!(sql.contains("target_is = 'dev'"), "{sql}");
        assert!(sql.contains("incr = False"), "{sql}"); // Jinja booleans render Python-style
    }

    #[test]
    fn unknown_var_without_default_is_guidance() {
        let err = expand_jinja("select {{ var('missing') }}", &inputs()).unwrap_err();
        let message = err.to_message();
        assert!(message.contains("missing"), "{message}");
        assert!(message.contains("real dbt compile"), "{message}");
    }
}
