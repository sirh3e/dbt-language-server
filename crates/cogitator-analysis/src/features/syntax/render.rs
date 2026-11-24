//! The model rendered as dbt renders it, for the SQL check of a model
//! whose Jinja is more than `{{ … }}`: `{% if is_incremental() %}`
//! blocks, `{% for %}` loops, `{% set %}` blocks, macros. It is
//! dbt.expandJinja's renderer (the project's macros, `ref`, `source`,
//! `var`, `env_var`, `config`, `target`, `this`) made strict: anything it
//! does not know — an adapter call, `run_query`, a package namespace, a
//! var with no value — fails the render, and a model that cannot be
//! rendered is not checked. Pure: the shell assembles the inputs once
//! per project state (the macro files are read there).

use minijinja::value::Value;
use minijinja::{Error, UndefinedBehavior};

use super::super::jinja_preview::{environment, PreviewInputs, Relation};

/// Render budget (minijinja fuel: instructions) per rendering.
pub const RENDER_FUEL: u64 = 2_000_000;

/// The renderer of one project state.
pub struct SyntaxRenderer {
    inputs: PreviewInputs,
    /// The macro files that parse, made parseable (dbt's own tags) and
    /// concatenated ahead of every model.
    macros: String,
}

impl SyntaxRenderer {
    pub fn new(mut inputs: PreviewInputs) -> Self {
        let mut macros = String::new();
        let scratch = minijinja::Environment::new();
        for source in std::mem::take(&mut inputs.macro_sources) {
            // a file minijinja cannot parse is left out: what calls its
            // macros fails to render and is not checked
            let Some(text) = crate::features::rename_local::jinja::parseable(&source) else {
                continue;
            };
            if scratch.template_from_str(&text).is_err() {
                continue;
            }
            macros.push_str(&text);
            macros.push('\n');
        }
        SyntaxRenderer { inputs, macros }
    }

    /// `body` rendered with `is_incremental()` answering `incremental`
    /// and `this` the model's relation; `None` when it cannot be.
    pub(crate) fn render(&self, body: &str, model: &str, incremental: bool) -> Option<String> {
        let mut env = environment(&self.inputs);
        env.set_undefined_behavior(UndefinedBehavior::Strict);
        env.set_fuel(Some(RENDER_FUEL));
        env.add_function("is_incremental", move || {
            Ok::<Value, Error>(Value::from(incremental))
        });
        let this = self
            .inputs
            .model_relations
            .get(model)
            .cloned()
            .unwrap_or_else(|| model.to_string());
        env.add_global("this", Relation::value(&this));
        // dbt's compile-time context: `execute` is true while dbt
        // compiles, and these two only ever land in literals
        env.add_global("execute", Value::from(true));
        env.add_global(
            "invocation_id",
            Value::from("00000000-0000-0000-0000-000000000000"),
        );
        env.add_global("run_started_at", Value::from("2000-01-01 00:00:00+00:00"));
        // a generic test's file has dbt's own `{% test %}` blocks
        let body = crate::features::rename_local::jinja::parseable(body)
            .unwrap_or_else(|| body.to_string());
        let mut template = String::with_capacity(self.macros.len() + body.len());
        template.push_str(&self.macros);
        template.push_str(&body);
        match env.render_str(&template, minijinja::context! {}) {
            Ok(sql) => Some(sql[self.rendered_prefix(&sql)..].to_string()),
            Err(err) => {
                let line = err
                    .line()
                    .and_then(|n| template.lines().nth(n.saturating_sub(1)))
                    .unwrap_or("")
                    .trim();
                tracing::trace!(target: "cogitator::syntax", "render declined: {err} | {line}");
                None
            }
        }
    }

    /// Where the model's own output starts: the macro definitions render
    /// to whitespace alone.
    fn rendered_prefix(&self, sql: &str) -> usize {
        sql.len() - sql.trim_start_matches(['\n', ' ', '\t', '\r']).len()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    pub(crate) fn renderer(macros: &[&str]) -> SyntaxRenderer {
        SyntaxRenderer::new(PreviewInputs {
            macro_sources: macros.iter().map(|m| m.to_string()).collect(),
            model_relations: HashMap::from([("events".to_string(), "db.main.events".to_string())]),
            source_relations: HashMap::new(),
            vars: HashMap::from([("cols".to_string(), serde_json::json!(["a", "b"]))]),
            target_name: "dev".to_string(),
            is_incremental: false,
            this: None,
            env: HashMap::new(),
        })
    }

    #[rstest::rstest]
    #[case(
        "select 1 {% if is_incremental() %}where x > (select max(x) from {{ this }}){% endif %}",
        false,
        Some("select 1 ")
    )]
    #[case(
        "select 1 {% if is_incremental() %}where x > (select max(x) from {{ this }}){% endif %}",
        true,
        Some("select 1 where x > (select max(x) from db.main.events)")
    )]
    #[case(
        "select {% for c in var('cols') %}{{ c }}{% if not loop.last %}, {% endif %}{% endfor %} from t",
        false,
        Some("select a, b from t")
    )]
    #[case("select {{ shout('x') }}", false, Some("select X"))]
    #[case("select {{ dbt_utils.star(ref('t')) }}", false, None)]
    #[case("select {{ run_query('x') }}", false, None)]
    #[case("{% for i in range(100000000) %}x{% endfor %}", false, None)]
    fn renders_as_dbt_or_declines(
        #[case] body: &str,
        #[case] incremental: bool,
        #[case] expected: Option<&str>,
    ) {
        let r = renderer(&["{% macro shout(s) %}{{ s | upper }}{% endmacro %}"]);
        assert_eq!(r.render(body, "events", incremental).as_deref(), expected);
    }

    #[rstest::rstest]
    #[case(
        "{{ this.identifier }} {{ this.schema }} {{ this }}",
        "events main db.main.events"
    )]
    #[case("{{ ref('events').identifier }} {{ ref('x', v=2) }}", "events x_v2")]
    #[case("{% if execute %}yes{% endif %}", "yes")]
    #[case("{% test not_empty(model) %}select 1{% endtest %}", "")]
    fn renders_dbt_context(#[case] body: &str, #[case] expected: &str) {
        assert_eq!(
            renderer(&[]).render(body, "events", false).as_deref(),
            Some(expected)
        );
    }

    #[test]
    fn a_macro_file_that_does_not_parse_is_left_out() {
        let r = renderer(&["{% macro broken( %}", "{% macro ok() %}1{% endmacro %}"]);
        assert_eq!(
            r.render("select {{ ok() }}", "m", false).as_deref(),
            Some("select 1")
        );
    }
}
