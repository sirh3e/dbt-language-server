//! What is wrong with one call's arguments, given the macro's parameters
//! and body facts. Pure; the case table is `judge_tests.rs`.

use cogitator_parser::jinja_scan::MacroParam;
use cogitator_parser::macro_body::{CallShape, MacroBody};

/// One problem of a call, with the byte offset (into the call's source)
/// of the keyword it is about, when it is about one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// More positional arguments than parameters.
    TooMany { given: usize, takes: usize },
    /// Required parameters no argument fills.
    Missing { names: Vec<String> },
    /// A keyword no parameter has; `suggestions` closest first.
    UnknownKwarg {
        name: String,
        offset: u32,
        suggestions: Vec<String>,
    },
    /// A keyword given twice, or naming a parameter a positional fills.
    DuplicateKwarg { name: String, offset: u32 },
}

pub fn judge(params: &[MacroParam], body: &MacroBody, shape: &CallShape) -> Vec<Problem> {
    let mut out = Vec::new();
    if shape.splat {
        return out; // argument count unknowable
    }
    if !body.reads_varargs && shape.positional > params.len() {
        out.push(Problem::TooMany {
            given: shape.positional,
            takes: params.len(),
        });
    }
    let mut seen: Vec<&str> = Vec::new();
    for (name, offset) in &shape.kwargs {
        let position = params.iter().position(|p| p.name == *name);
        match position {
            None if !body.reads_kwargs => out.push(Problem::UnknownKwarg {
                name: name.clone(),
                offset: *offset,
                suggestions: cogitator_error::guidance::closest_matches(
                    name,
                    params.iter().map(|p| p.name.as_str()),
                ),
            }),
            Some(i) if i < shape.positional || seen.contains(&name.as_str()) => {
                out.push(Problem::DuplicateKwarg {
                    name: name.clone(),
                    offset: *offset,
                });
            }
            _ => {}
        }
        seen.push(name);
    }
    let missing: Vec<String> = params
        .iter()
        .enumerate()
        .filter(|(i, p)| {
            p.default.is_none()
                && *i >= shape.positional
                && !shape.kwargs.iter().any(|(k, _)| *k == p.name)
        })
        .map(|(_, p)| p.name.clone())
        .collect();
    if !missing.is_empty() {
        out.push(Problem::Missing { names: missing });
    }
    out
}

#[cfg(test)]
#[path = "judge_tests.rs"]
mod tests;
