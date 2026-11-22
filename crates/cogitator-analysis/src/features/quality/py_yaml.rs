//! A YAML scalar as dbt sees it. dbt reads properties YAML with PyYAML's
//! `SafeLoader`, which follows YAML 1.1 (`yes`/`on` are booleans, `010`
//! is octal, `1e3` is a string), while saphyr follows YAML 1.2. A test's
//! dbt name is built from Python's `str()` of its arguments, so the plain
//! scalars are resolved again here by PyYAML's implicit resolvers, and
//! the values written back as Python writes them.

use std::sync::LazyLock;

use cogitator_schemas::yaml::{entries, items};
use regex::Regex;
use saphyr::{MarkedYaml, Scalar, YamlData};

/// The document text, by line, to tell a plain scalar from a quoted or
/// block one (saphyr keeps no scalar style once it parsed the value).
pub struct Source<'t> {
    lines: Vec<&'t str>,
}

impl<'t> Source<'t> {
    pub fn new(text: &'t str) -> Self {
        Self {
            lines: text.lines().collect(),
        }
    }

    /// The first character of a node in the text.
    fn first_char(&self, node: &MarkedYaml<'_>) -> Option<char> {
        let line = self.lines.get(node.span.start.line().checked_sub(1)?)?;
        line.chars().nth(node.span.start.col())
    }

    /// The node's own text when it sits on one line.
    fn raw(&self, node: &MarkedYaml<'_>) -> Option<String> {
        let (start, end) = (node.span.start, node.span.end);
        if start.line() != end.line() {
            return None;
        }
        let line = self.lines.get(start.line().checked_sub(1)?)?;
        Some(
            line.chars()
                .skip(start.col())
                .take(end.col() - start.col())
                .collect(),
        )
    }
}

/// A scalar as PyYAML's SafeLoader constructs it.
#[derive(Debug, Clone, PartialEq)]
enum Py {
    None,
    Bool(bool),
    Int(i128),
    Float(f64),
    Str(String),
}

/// PyYAML's implicit resolvers (resolver.py), anchored.
static BOOL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:yes|Yes|YES|no|No|NO|true|True|TRUE|false|False|FALSE|on|On|ON|off|Off|OFF)$")
        .expect("bool regex")
});
static INT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?:[-+]?0b[0-1_]+|[-+]?0[0-7_]+|[-+]?(?:0|[1-9][0-9_]*)|[-+]?0x[0-9a-fA-F_]+|[-+]?[1-9][0-9_]*(?::[0-5]?[0-9])+)$",
    )
    .expect("int regex")
});
static FLOAT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?:[-+]?(?:[0-9][0-9_]*)\.[0-9_]*(?:[eE][-+][0-9]+)?|\.[0-9][0-9_]*(?:[eE][-+][0-9]+)?|[-+]?[0-9][0-9_]*(?::[0-5]?[0-9])+\.[0-9_]*|[-+]?\.(?:inf|Inf|INF)|\.(?:nan|NaN|NAN))$",
    )
    .expect("float regex")
});

/// Resolve a plain scalar's text as YAML 1.1 does.
fn resolve(text: &str) -> Py {
    if matches!(text, "" | "~" | "null" | "Null" | "NULL") {
        return Py::None;
    }
    if BOOL.is_match(text) {
        return Py::Bool(matches!(
            text,
            "yes" | "Yes" | "YES" | "true" | "True" | "TRUE" | "on" | "On" | "ON"
        ));
    }
    if INT.is_match(text) {
        if let Some(i) = construct_int(text) {
            return Py::Int(i);
        }
    }
    if FLOAT.is_match(text) {
        if let Some(f) = construct_float(text) {
            return Py::Float(f);
        }
    }
    Py::Str(text.to_string())
}

/// SafeConstructor.construct_yaml_int.
fn construct_int(text: &str) -> Option<i128> {
    let text = text.replace('_', "");
    let (sign, digits) = match text.strip_prefix('-') {
        Some(rest) => (-1, rest),
        None => (1, text.strip_prefix('+').unwrap_or(&text)),
    };
    let value = if digits == "0" {
        0
    } else if let Some(bin) = digits.strip_prefix("0b") {
        i128::from_str_radix(bin, 2).ok()?
    } else if let Some(hex) = digits.strip_prefix("0x") {
        i128::from_str_radix(hex, 16).ok()?
    } else if digits.starts_with('0') {
        i128::from_str_radix(digits, 8).ok()?
    } else if digits.contains(':') {
        digits.split(':').try_fold(0i128, |acc, part| {
            Some(acc * 60 + part.parse::<i128>().ok()?)
        })?
    } else {
        digits.parse().ok()?
    };
    Some(sign * value)
}

/// SafeConstructor.construct_yaml_float.
fn construct_float(text: &str) -> Option<f64> {
    let text = text.replace('_', "").to_lowercase();
    let (sign, body) = match text.strip_prefix('-') {
        Some(rest) => (-1.0, rest.to_string()),
        None => (1.0, text.strip_prefix('+').unwrap_or(&text).to_string()),
    };
    let value = if body == ".inf" {
        f64::INFINITY
    } else if body == ".nan" {
        f64::NAN
    } else if body.contains(':') {
        body.split(':').try_fold(0.0, |acc, part| {
            Some(acc * 60.0 + part.parse::<f64>().ok()?)
        })?
    } else {
        body.parse().ok()?
    };
    Some(sign * value)
}

/// What PyYAML would have constructed for a scalar node.
fn scalar(node: &MarkedYaml<'_>, src: &Source<'_>) -> Option<Py> {
    let quoted_or_block = || {
        src.first_char(node)
            .is_some_and(|c| matches!(c, '\'' | '"' | '|' | '>'))
    };
    match &node.data {
        YamlData::Value(Scalar::String(s)) | YamlData::Representation(s, _, _) => {
            Some(if quoted_or_block() {
                Py::Str(s.to_string())
            } else {
                resolve(s)
            })
        }
        YamlData::Value(other) => Some(match src.raw(node) {
            Some(raw) => resolve(&raw),
            None => match other {
                Scalar::Null => Py::None,
                Scalar::Boolean(b) => Py::Bool(*b),
                Scalar::Integer(i) => Py::Int(*i as i128),
                Scalar::FloatingPoint(f) => Py::Float(f.into_inner()),
                Scalar::String(s) => Py::Str(s.to_string()),
            },
        }),
        YamlData::Tagged(_, inner) => scalar(inner, src),
        _ => None,
    }
}

/// Python's `str()` of a YAML value.
pub fn py_str(node: &MarkedYaml<'_>, src: &Source<'_>) -> String {
    match scalar(node, src) {
        Some(Py::Str(s)) => s,
        _ => py_repr(node, src),
    }
}

/// Python's `repr()` of a YAML value.
pub fn py_repr(node: &MarkedYaml<'_>, src: &Source<'_>) -> String {
    if let Some(value) = scalar(node, src) {
        return match value {
            Py::None => "None".to_string(),
            Py::Bool(true) => "True".to_string(),
            Py::Bool(false) => "False".to_string(),
            Py::Int(i) => i.to_string(),
            Py::Float(f) => py_float(f),
            Py::Str(s) => str_repr(&s),
        };
    }
    match &node.data {
        YamlData::Sequence(_) => format!(
            "[{}]",
            items(node)
                .map(|v| py_repr(v, src))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        YamlData::Mapping(_) => format!(
            "{{{}}}",
            entries(node)
                .map(|(k, v)| format!("{}: {}", py_repr(k, src), py_repr(v, src)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        _ => "None".to_string(),
    }
}

/// `repr(str)`: single quotes unless the text holds one and no double.
fn str_repr(s: &str) -> String {
    if s.contains('\'') && !s.contains('"') {
        format!("\"{s}\"")
    } else {
        format!("'{}'", s.replace('\'', "\\'"))
    }
}

/// `repr(float)`: the shortest digits, in fixed notation for decimal
/// exponents in [-4, 16) and scientific (two exponent digits) otherwise.
pub fn py_float(f: f64) -> String {
    if f.is_nan() {
        return "nan".to_string();
    }
    if f.is_infinite() {
        return if f > 0.0 { "inf" } else { "-inf" }.to_string();
    }
    if f == 0.0 {
        return if f.is_sign_negative() { "-0.0" } else { "0.0" }.to_string();
    }
    let sci = format!("{f:e}");
    let (mantissa, exp) = sci.split_once('e').expect("scientific form");
    let exp: i32 = exp.parse().expect("exponent");
    if (-4..16).contains(&exp) {
        let fixed = f.to_string();
        if fixed.contains('.') {
            fixed
        } else {
            format!("{fixed}.0")
        }
    } else {
        let sign = if exp < 0 { '-' } else { '+' };
        format!("{mantissa}e{sign}{:02}", exp.abs())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use saphyr::LoadableYamlNode;

    /// `str(yaml.safe_load("x: <raw>")["x"])`, from PyYAML 6.0.
    #[test]
    fn scalars_resolve_as_pyyaml() {
        let cases: &[(&str, &str)] = &[
            ("yes", "True"),
            ("'yes'", "yes"),
            ("Off", "False"),
            ("~", "None"),
            ("null", "None"),
            ("0x1F", "31"),
            ("010", "8"),
            ("0b101", "5"),
            ("1_000", "1000"),
            ("1:30", "90"),
            ("1e3", "1e3"),
            ("1.5e+3", "1500.0"),
            (".5", "0.5"),
            ("1.0e+20", "1e+20"),
            ("0.0001", "0.0001"),
            ("0.00001", "1e-05"),
            ("-7", "-7"),
            ("2.0", "2.0"),
            ("3.25", "3.25"),
            ("0o17", "0o17"),
            ("\"{{ var('v') }}\"", "{{ var('v') }}"),
            ("[1, 'two', true]", "[1, 'two', True]"),
            ("{x: \"it's\"}", "{'x': \"it's\"}"),
        ];
        for (raw, expected) in cases {
            let text = format!("x: {raw}\n");
            let docs = MarkedYaml::load_from_str(&text).unwrap();
            let value = cogitator_schemas::yaml::get(&docs[0], "x").unwrap();
            assert_eq!(py_str(value, &Source::new(&text)), *expected, "for {raw}");
        }
    }
}
