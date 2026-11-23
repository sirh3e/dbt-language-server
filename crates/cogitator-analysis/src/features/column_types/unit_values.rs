//! `unit-test-value-type`: a unit test's `given:` or `expect:` row value
//! that cannot be a value of the column's proven type — `'abc'` for an
//! integer, `maybe` for a boolean. A Hint, and only for clear cases: dbt
//! casts row values, so a date written as a string is fine.
//!
//! Rows are read in dbt's default `dict` format; `csv` and `sql` fixtures
//! are not checked. `expect` uses the tested model's types, a `given`
//! input the types of the model it `ref`s (sources are not checked).

use cogitator_error::guidance::Guidance;
use cogitator_lsp::{diagnostic_severity, Diagnostic};
use cogitator_schemas::yaml::{as_str, entries, get, items, range_of};
use saphyr::{LoadableYamlNode, MarkedYaml, Scalar, YamlData};

use super::basis::ModelColumnTypes;
use super::compare::render;
use cogitator_ir::SqlType;

pub const VALUE_TYPE: &str = "unit-test-value-type";

/// The impossible row values of every unit test in `yaml_text`.
pub fn unit_test_value_diagnostics(
    yaml_text: &str,
    dialect: &cogitator_dialects::Dialect,
    types: &dyn Fn(&str) -> Option<ModelColumnTypes>,
) -> Vec<Diagnostic> {
    let Ok(docs) = MarkedYaml::load_from_str(yaml_text) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for test in docs
        .iter()
        .filter_map(|d| get(d, "unit_tests"))
        .flat_map(items)
    {
        let Some(model) = get(test, "model").and_then(as_str) else {
            continue;
        };
        if let Some(expect) = get(test, "expect") {
            out.extend(check_rows(expect, model, dialect, types));
        }
        for input in get(test, "given").into_iter().flat_map(items) {
            let Some(target) = get(input, "input").and_then(as_str).and_then(ref_name) else {
                continue;
            };
            out.extend(check_rows(input, &target, dialect, types));
        }
    }
    out
}

/// `ref('m')` / `ref("m")` → `m`; versioned or package refs and sources
/// are not checked.
fn ref_name(input: &str) -> Option<String> {
    let inner = input.trim().strip_prefix("ref(")?.strip_suffix(')')?.trim();
    let name = inner
        .strip_prefix('\'')
        .and_then(|s| s.strip_suffix('\''))
        .or_else(|| inner.strip_prefix('"').and_then(|s| s.strip_suffix('"')))?;
    (!name.contains(['\'', '"', ','])).then(|| name.to_string())
}

fn check_rows(
    holder: &MarkedYaml<'_>,
    model: &str,
    dialect: &cogitator_dialects::Dialect,
    types: &dyn Fn(&str) -> Option<ModelColumnTypes>,
) -> Vec<Diagnostic> {
    let format = get(holder, "format").and_then(as_str).unwrap_or("dict");
    if !format.eq_ignore_ascii_case("dict") {
        return Vec::new();
    }
    let Some(rows) = get(holder, "rows") else {
        return Vec::new();
    };
    let Some(typed) = types(model) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for row in items(rows) {
        for (key, value) in entries(row) {
            let Some(name) = as_str(key) else {
                continue;
            };
            let Some(column) = typed.column(name).filter(|c| c.is_proven()) else {
                continue;
            };
            if fits(&value.data, &column.sql_type) {
                continue;
            }
            let spelled = render(&column.sql_type, dialect).unwrap_or(column.description.clone());
            let guidance = Guidance::new(format!(
                "'{model}.{name}' is {spelled} ({}); this value cannot be one",
                column.why
            ))
            .fix(format!("write a {spelled} value, or null."));
            out.push(Diagnostic::plain(
                range_of(value),
                guidance.to_diagnostic(),
                diagnostic_severity::HINT,
                VALUE_TYPE,
                "cogitator",
            ));
        }
    }
    out
}

/// Whether a YAML value could be cast to `ty`. Unsure means yes.
pub(crate) fn fits<'a>(value: &YamlData<'a, MarkedYaml<'a>>, ty: &SqlType) -> bool {
    let scalar = match value {
        YamlData::Value(s) => s,
        YamlData::Sequence(_) | YamlData::Mapping(_) => {
            return !matches!(
                ty,
                SqlType::Bool
                    | SqlType::Int { .. }
                    | SqlType::Decimal { .. }
                    | SqlType::Float { .. }
                    | SqlType::Date
                    | SqlType::Time
                    | SqlType::Timestamp { .. }
            );
        }
        _ => return true,
    };
    match (scalar, ty) {
        (Scalar::Null, _) => true,
        (Scalar::Boolean(_), SqlType::Bool | SqlType::Text) => true,
        (
            Scalar::Boolean(_),
            SqlType::Int { .. }
            | SqlType::Decimal { .. }
            | SqlType::Float { .. }
            | SqlType::Date
            | SqlType::Time
            | SqlType::Timestamp { .. },
        ) => false,
        (Scalar::Integer(i), SqlType::Bool) => *i == 0 || *i == 1,
        (Scalar::FloatingPoint(_), SqlType::Bool) => false,
        (Scalar::String(s), SqlType::Int { .. }) => s.trim().parse::<i128>().is_ok(),
        (Scalar::String(s), SqlType::Decimal { .. } | SqlType::Float { .. }) => {
            s.trim().parse::<f64>().is_ok()
        }
        (Scalar::String(s), SqlType::Bool) => matches!(
            s.trim().to_ascii_lowercase().as_str(),
            "true" | "false" | "t" | "f" | "yes" | "no" | "y" | "n" | "1" | "0" | "on" | "off"
        ),
        _ => true,
    }
}
