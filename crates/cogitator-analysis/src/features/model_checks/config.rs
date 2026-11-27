//! The `config(...)` checks: each keyword argument with its spans, and
//! what dbt would make of its key and its value.

use cogitator_error::guidance::closest_matches;
use cogitator_parser::jinja_spans::{blanked, NOT_CODE, NOT_SQL};
use cogitator_sql_front::{config_values, ConfigValue};

use super::{
    finding, DbtContext, Finding, Fix, CONFIG_KEY_CODE, CONFIG_VALUE_CODE, EMPTY_MODEL_CODE, ERROR,
    MATERIALIZATION_CODE, NEVER_INCREMENTAL_CODE, WARNING,
};

/// The materializations dbt itself provides for models.
pub(crate) const BUILTIN_MATERIALIZATIONS: &[&str] = &[
    "view",
    "table",
    "incremental",
    "ephemeral",
    "materialized_view",
];

/// Materializations a common adapter provides; they are never reported,
/// as the adapter's macros are not in the project.
const ADAPTER_MATERIALIZATIONS: &[&str] = &["dynamic_table", "external", "clone"];

/// The materializations whose models `is_incremental()` is false in.
const NEVER_INCREMENTAL: &[&str] = &["view", "table", "ephemeral", "materialized_view"];

/// The config keys dbt reads for a model, and the ones the common
/// adapters read. A key outside the list is reported only when it is a
/// near miss of one in it: adapters add keys of their own.
pub(crate) const KNOWN_KEYS: &[&str] = &[
    // dbt
    "access",
    "alias",
    "batch_size",
    "begin",
    "column_types",
    "concurrent_batches",
    "contract",
    "database",
    "docs",
    "enabled",
    "event_time",
    "full_refresh",
    "grants",
    "group",
    "incremental_predicates",
    "incremental_strategy",
    "lookback",
    "materialized",
    "merge_exclude_columns",
    "merge_update_columns",
    "meta",
    "on_configuration_change",
    "on_schema_change",
    "persist_docs",
    "post_hook",
    "pre_hook",
    "quoting",
    "schema",
    "sql_header",
    "tags",
    "unique_key",
    // catalogs and table formats
    "catalog",
    "catalog_name",
    "table_format",
    // postgres, redshift
    "indexes",
    "unlogged",
    "sort",
    "sort_type",
    "dist",
    "bind",
    "backup",
    "auto_refresh",
    // snowflake
    "automatic_clustering",
    "cluster_by",
    "copy_grants",
    "query_tag",
    "secure",
    "snowflake_warehouse",
    "target_lag",
    "transient",
    "warehouse",
    // bigquery
    "hours_to_expiration",
    "kms_key_name",
    "labels",
    "partition_by",
    "partition_expiration_days",
    "partitions",
    "require_partition_filter",
    // spark, databricks
    "buckets",
    "clustered_by",
    "file_format",
    "liquid_clustered_by",
    "location_root",
    "options",
    "tblproperties",
    // duckdb
    "delimiter",
    "format",
    "glue_register",
    "location",
    "plugin",
    // python models
    "packages",
    "python_version",
    "submission_method",
];

const ON_SCHEMA_CHANGE: &[&str] = &["ignore", "fail", "append_new_columns", "sync_all_columns"];
const ACCESS: &[&str] = &["private", "protected", "public"];
const INCREMENTAL_STRATEGIES: &[&str] = &[
    "append",
    "merge",
    "delete+insert",
    "insert_overwrite",
    "microbatch",
    "replace_where",
];
/// Keys dbt reads as booleans.
const BOOLEAN_KEYS: &[&str] = &["enabled", "full_refresh"];

/// One keyword argument of a `config(...)` call.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Kwarg {
    pub key: String,
    /// The key's byte span.
    pub key_span: (usize, usize),
    /// The value's byte span, quotes included.
    pub value_span: (usize, usize),
    /// The value, when it is constant all the way down.
    pub value: Option<ConfigValue>,
    /// A string value's span inside its quotes.
    pub string_span: Option<(usize, usize)>,
}

/// One `config(...)` call: its span and its keyword arguments.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Call {
    pub span: (usize, usize),
    pub kwargs: Vec<Kwarg>,
}

/// The `config(...)` calls of a model's text, in file order.
pub(crate) fn calls(text: &str) -> Vec<Call> {
    crate::util::config_call_spans(text)
        .into_iter()
        .map(|span| Call {
            span,
            kwargs: kwargs(text, span),
        })
        .collect()
}

/// The keyword arguments of the call at `span` (from `config` to past
/// its `)`), with their constant values where they have one.
fn kwargs(text: &str, (start, end): (usize, usize)) -> Vec<Kwarg> {
    let call = &text[start..end];
    let values = config_values(call);
    let Some(open) = call.find('(') else {
        return Vec::new();
    };
    arguments(call, open + 1, call.len().saturating_sub(1))
        .into_iter()
        .filter_map(|(a, b)| {
            let arg = &call[a..b];
            let lead = arg.len() - arg.trim_start().len();
            let key_len = arg[lead..]
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(arg.len() - lead);
            let key = &arg[lead..lead + key_len];
            let after = &arg[lead + key_len..];
            let eq = after.len() - after.trim_start().len();
            let rest = &after[eq..];
            if key.is_empty() || !rest.starts_with('=') || rest.starts_with("==") {
                return None;
            }
            let value_text = &rest[1..];
            let value_lead = value_text.len() - value_text.trim_start().len();
            let value_start = a + lead + key_len + eq + 1 + value_lead;
            let value_end = a + arg.trim_end().len();
            let value = &call[value_start..value_end.max(value_start)];
            let string_span =
                quoted(value).map(|(s, e)| (start + value_start + s, start + value_start + e));
            Some(Kwarg {
                key: key.to_string(),
                key_span: (start + a + lead, start + a + lead + key_len),
                value_span: (start + value_start, start + value_end.max(value_start)),
                value: values
                    .iter()
                    .find(|(k, _)| k == key)
                    .map(|(_, v)| v.clone()),
                string_span,
            })
        })
        .collect()
}

/// The byte spans (relative to `call`) of the arguments between `from`
/// and `to`, split on the commas outside brackets and strings.
fn arguments(call: &str, from: usize, to: usize) -> Vec<(usize, usize)> {
    let bytes = call.as_bytes();
    let (mut depth, mut quote, mut out, mut arg_start) = (0usize, None::<u8>, Vec::new(), from);
    let mut i = from;
    while i < to {
        let b = bytes[i];
        match quote {
            Some(_) if b == b'\\' => i += 1,
            Some(q) if b == q => quote = None,
            Some(_) => {}
            None if b == b'\'' || b == b'"' => quote = Some(b),
            None if matches!(b, b'(' | b'[' | b'{') => depth += 1,
            None if matches!(b, b')' | b']' | b'}') => depth = depth.saturating_sub(1),
            None if b == b',' && depth == 0 => {
                out.push((arg_start, i));
                arg_start = i + 1;
            }
            None => {}
        }
        i += 1;
    }
    if call[arg_start..to].trim().is_empty() {
        return out;
    }
    out.push((arg_start, to));
    out
}

/// The span inside the quotes of a plain string literal.
fn quoted(value: &str) -> Option<(usize, usize)> {
    let quote = value.chars().next().filter(|c| *c == '\'' || *c == '"')?;
    (value.len() >= 2 && value.ends_with(quote) && !value[1..value.len() - 1].contains(quote))
        .then_some((1, value.len() - 1))
}

/// The key and value checks of every call.
pub(crate) fn check(ctx: &DbtContext, calls: &[Call]) -> Vec<Finding> {
    calls
        .iter()
        .flat_map(|call| call.kwargs.iter())
        .flat_map(|kwarg| check_key(kwarg).into_iter().chain(check_value(ctx, kwarg)))
        .collect()
}

fn check_key(kwarg: &Kwarg) -> Option<Finding> {
    if KNOWN_KEYS.contains(&kwarg.key.as_str()) {
        return None;
    }
    let near = closest_matches(&kwarg.key, KNOWN_KEYS.iter().copied());
    (!near.is_empty()).then(|| {
        finding(
            kwarg.key_span,
            CONFIG_KEY_CODE,
            WARNING,
            format!("config key '{}' is not one dbt reads", kwarg.key),
        )
        .because("dbt keeps an unknown key without acting on it")
        .fixed_by(Fix::Replace(near))
    })
}

fn check_value(ctx: &DbtContext, kwarg: &Kwarg) -> Option<Finding> {
    let key = kwarg.key.as_str();
    if BOOLEAN_KEYS.contains(&key) {
        return check_boolean(kwarg);
    }
    let (Some(ConfigValue::Str(value)), Some(span)) = (&kwarg.value, kwarg.string_span) else {
        return None;
    };
    match key {
        "materialized" => check_materialization(ctx, value, span),
        "on_schema_change" => one_of(value, span, key, ON_SCHEMA_CHANGE, WARNING)
            .map(|f| f.because("dbt ignores it and falls back to 'ignore'")),
        "access" => one_of(value, span, key, ACCESS, ERROR)
            .map(|f| f.because("dbt refuses the project at parse time")),
        "incremental_strategy" => check_strategy(ctx, value, span),
        _ => None,
    }
}

/// `enabled='false'`: a string, which Jinja reads as true.
fn check_boolean(kwarg: &Kwarg) -> Option<Finding> {
    let Some(ConfigValue::Str(value)) = &kwarg.value else {
        return None;
    };
    let fixed = match value.to_ascii_lowercase().as_str() {
        "true" => "true",
        "false" => "false",
        _ => return None,
    };
    let (start, end) = kwarg.value_span;
    Some(
        finding(
            kwarg.value_span,
            CONFIG_VALUE_CODE,
            WARNING,
            format!("{} is the string '{value}', not a boolean", kwarg.key),
        )
        .because("dbt expects true or false here, and any non-empty string reads as true")
        .fixed_by(Fix::Rewrite {
            title: format!("Use the boolean {fixed}"),
            start,
            end,
            text: fixed.to_string(),
        }),
    )
}

fn check_materialization(ctx: &DbtContext, value: &str, span: (usize, usize)) -> Option<Finding> {
    let known = BUILTIN_MATERIALIZATIONS
        .iter()
        .chain(ADAPTER_MATERIALIZATIONS)
        .copied()
        .chain(ctx.materializations.iter().map(String::as_str));
    if known.clone().any(|m| m == value) {
        return None;
    }
    let near = closest_matches(value, known);
    if near.is_empty() {
        return Some(
            finding(
                span,
                MATERIALIZATION_CODE,
                WARNING,
                format!("materialization '{value}' is not defined by dbt or this project"),
            )
            .because("dbt fails to build the model unless the adapter provides it"),
        );
    }
    Some(
        finding(
            span,
            MATERIALIZATION_CODE,
            ERROR,
            format!("materialization '{value}' does not exist"),
        )
        .because("dbt fails to build the model")
        .fixed_by(Fix::Replace(near)),
    )
}

/// A custom strategy is a `get_incremental_<name>_sql` macro.
fn check_strategy(ctx: &DbtContext, value: &str, span: (usize, usize)) -> Option<Finding> {
    let macro_name = format!("get_incremental_{value}_sql");
    let custom = ctx
        .macro_detail_map
        .values()
        .any(|macros| macros.contains_key(&macro_name));
    if custom || INCREMENTAL_STRATEGIES.contains(&value) {
        return None;
    }
    let near = closest_matches(value, INCREMENTAL_STRATEGIES.iter().copied());
    (!near.is_empty()).then(|| {
        finding(
            span,
            CONFIG_VALUE_CODE,
            WARNING,
            format!("incremental_strategy '{value}' is not a strategy dbt knows"),
        )
        .because(format!("no macro {macro_name} defines it"))
        .fixed_by(Fix::Replace(near))
    })
}

fn one_of(
    value: &str,
    span: (usize, usize),
    key: &str,
    allowed: &[&str],
    severity: i32,
) -> Option<Finding> {
    if allowed.contains(&value) {
        return None;
    }
    let mut near = closest_matches(value, allowed.iter().copied());
    if near.is_empty() {
        near = allowed.iter().map(|s| s.to_string()).collect();
    }
    Some(
        finding(
            span,
            CONFIG_VALUE_CODE,
            severity,
            format!("{key} '{value}' is not one of {}", allowed.join(", ")),
        )
        .fixed_by(Fix::Replace(near)),
    )
}

/// The materialization the model's own SQL sets, the last call winning.
fn materialized(calls: &[Call]) -> Option<&str> {
    calls
        .iter()
        .rev()
        .flat_map(|c| c.kwargs.iter().rev())
        .filter(|k| k.key == "materialized")
        .filter_map(|k| match &k.value {
            Some(ConfigValue::Str(s)) => Some(s.as_str()),
            _ => None,
        })
        .next()
}

/// `is_incremental()` in a model whose SQL says it is not incremental:
/// the block never runs.
pub(crate) fn never_incremental(text: &str, calls: &[Call]) -> Vec<Finding> {
    let Some(materialized) = materialized(calls).filter(|m| NEVER_INCREMENTAL.contains(m)) else {
        return Vec::new();
    };
    let live = blanked(text, NOT_CODE);
    live.match_indices("is_incremental")
        .filter(|(i, _)| {
            let before = live[..*i].chars().next_back();
            let after = live[i + "is_incremental".len()..].trim_start();
            !before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
                && after.starts_with('(')
        })
        .map(|(i, word)| {
            finding(
                (i, i + word.len()),
                NEVER_INCREMENTAL_CODE,
                WARNING,
                format!("is_incremental() is always false in a {materialized} model"),
            )
            .because(format!("the model is materialized as '{materialized}'"))
            .solved_by("materialize the model as 'incremental', or drop the block")
        })
        .collect()
}

/// A model with no SQL beyond comments and its `config(...)`: dbt runs
/// an empty statement and the build fails.
pub(crate) fn empty_model(text: &str) -> Vec<Finding> {
    let mut live = blanked(text, NOT_SQL).into_bytes();
    for (start, end) in crate::util::config_call_spans(text) {
        let open = text[..start].rfind("{{").unwrap_or(start);
        let close = text[end..].find("}}").map_or(end, |i| end + i + 2);
        live[open..close].fill(b' ');
    }
    if has_sql(&String::from_utf8_lossy(&live)) {
        return Vec::new();
    }
    let first_line = text.find('\n').unwrap_or(text.len());
    vec![finding(
        (0, first_line),
        EMPTY_MODEL_CODE,
        WARNING,
        "the model has no SQL".to_string(),
    )
    .because("dbt builds a model from its SELECT, and this one has none")
    .solved_by("write the model's SELECT, or delete the file")]
}

/// Whether anything but white space and SQL comments is left.
fn has_sql(text: &str) -> bool {
    let mut rest = text;
    loop {
        rest = rest.trim_start();
        if let Some(after) = rest.strip_prefix("--") {
            rest = after.find('\n').map_or("", |i| &after[i..]);
        } else if let Some(after) = rest.strip_prefix("/*") {
            rest = after.find("*/").map_or("", |i| &after[i + 2..]);
        } else {
            return !rest.is_empty();
        }
    }
}
