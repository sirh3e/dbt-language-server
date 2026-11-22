//! "Scaffold a unit test": a dbt ≥ 1.8 `unit_tests:` entry for a model in
//! its properties YAML — one `given:` input per ref()/source() the model
//! reads, each with one example row keyed by the columns the model reads
//! from it (lineage first, else the input's documented columns), and an
//! `expect:` row keyed by the model's output columns. Every value is a
//! `null` placeholder. Pure: the shell passes both texts in.

use cogitator_dialects::Dialect;
use cogitator_lsp::{CodeAction, DocumentChange, TextDocumentEdit, WorkspaceEdit};
use cogitator_parser::{DocumentParser, JinjaSqlParser, ParsedDocument, TokenKind};
use cogitator_schemas::yaml::{as_str, get, items};
use saphyr::{LoadableYamlNode, MarkedYaml};

use super::yaml_text::{block_end, indent, insert_after_line, is_content, plain_name};
use crate::context::ModelDetails;
use crate::lineage::{ModelLineage, SourceRef};
use crate::DbtContext;

/// A relation the model reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    Ref(String),
    Source(String, String),
}

impl Input {
    fn yaml(&self) -> String {
        match self {
            Input::Ref(name) => format!("ref('{name}')"),
            Input::Source(source, table) => format!("source('{source}', '{table}')"),
        }
    }
}

/// The model's ref()/source() relations, first occurrence first.
pub fn model_inputs(parsed: &ParsedDocument) -> Vec<Input> {
    let mut out: Vec<Input> = Vec::new();
    for (idx, token) in parsed.tokens.iter().enumerate() {
        let input = match token.kind {
            TokenKind::Ref => Input::Ref(parsed.literal(token).to_string()),
            TokenKind::SourceTable => {
                let Some(source) = parsed.lookback_find(idx, TokenKind::SourceName, 4) else {
                    continue;
                };
                Input::Source(
                    parsed.literal(source).to_string(),
                    parsed.literal(token).to_string(),
                )
            }
            _ => continue,
        };
        if !out.contains(&input) {
            out.push(input);
        }
    }
    out
}

/// The columns the model reads from `input`: those its lineage traces to
/// it, else every column the project documents for it.
fn input_columns(ctx: &DbtContext, input: &Input, lineage: Option<&ModelLineage>) -> Vec<String> {
    let mut traced: Vec<String> = Vec::new();
    for column in lineage.iter().flat_map(|l| l.columns.iter()) {
        for source in &column.sources {
            let name = match (source, input) {
                (SourceRef::Model { name, column }, Input::Ref(r)) if name == r => column,
                (
                    SourceRef::Source {
                        source,
                        table,
                        column,
                    },
                    Input::Source(s, t),
                ) if source == s && table == t => column,
                _ => continue,
            };
            if !name.contains('*') && !traced.iter().any(|t| t.eq_ignore_ascii_case(name)) {
                traced.push(name.clone());
            }
        }
    }
    if !traced.is_empty() {
        return traced;
    }
    let documented = match input {
        Input::Ref(name) => ctx.model_detail_map.get(name).map(|d| &d.columns),
        Input::Source(source, table) => ctx
            .source_detail_map
            .get(source)
            .and_then(|s| s.tables.get(table))
            .map(|t| &t.columns),
    };
    documented
        .into_iter()
        .flatten()
        .map(|c| c.name.clone())
        .collect()
}

/// The model's output columns: lineage's productions, else its YAML columns.
fn output_columns(details: &ModelDetails, lineage: Option<&ModelLineage>) -> Vec<String> {
    let produced: Vec<String> = lineage
        .iter()
        .flat_map(|l| l.columns.iter())
        .map(|c| c.name.clone())
        .filter(|n| !n.contains('*'))
        .collect();
    if !produced.is_empty() {
        return produced;
    }
    details.columns.iter().map(|c| c.name.clone()).collect()
}

/// `{a: null, b: null}`, keys quoted where a plain scalar would not do.
fn row(columns: &[String]) -> String {
    let keys: Vec<String> = columns
        .iter()
        .map(|c| match plain_name(c) {
            Some(plain) => format!("{plain}: null"),
            None => format!("{}: null", serde_json::Value::String(c.clone())),
        })
        .collect();
    format!("{{{}}}", keys.join(", "))
}

fn rows_lines(columns: &[String], out: &mut Vec<String>, pad: &str) {
    if columns.is_empty() {
        out.push(format!("{pad}rows: []"));
    } else {
        out.push(format!("{pad}rows:"));
        out.push(format!("{pad}  - {}", row(columns)));
    }
}

/// The entry's lines, unindented (the first begins `- `).
pub fn unit_test_lines(
    name: &str,
    model: &str,
    inputs: &[(Input, Vec<String>)],
    expect: &[String],
) -> Vec<String> {
    let mut out = vec![format!("- name: {name}"), format!("  model: {model}")];
    if inputs.is_empty() {
        out.push("  given: []".to_string());
    } else {
        out.push("  given:".to_string());
        for (input, columns) in inputs {
            out.push(format!("    - input: {}", input.yaml()));
            rows_lines(columns, &mut out, "      ");
        }
    }
    out.push("  expect:".to_string());
    rows_lines(expect, &mut out, "    ");
    out
}

/// Every unit test name the YAML defines.
fn existing_names(yaml: &str) -> Vec<String> {
    let Ok(docs) = MarkedYaml::load_from_str(yaml) else {
        return Vec::new();
    };
    docs.first()
        .and_then(|root| get(root, "unit_tests"))
        .into_iter()
        .flat_map(items)
        .filter_map(|u| get(u, "name").and_then(as_str).map(str::to_string))
        .collect()
}

fn fresh_name(model: &str, taken: &[String]) -> String {
    let base = format!("test_{model}");
    (1..)
        .map(|n| match n {
            1 => base.clone(),
            n => format!("{base}_{n}"),
        })
        .find(|candidate| !taken.contains(candidate))
        .expect("an unbounded range has a free name")
}

/// The scaffold action: `sql` is the model's text, `yaml` its properties
/// file's (at `yaml_uri`). None when the model has no properties file, or
/// its `unit_tests:` is written inline (`unit_tests: []`).
pub fn unit_test_scaffold_action(
    ctx: &DbtContext,
    model: &str,
    details: &ModelDetails,
    sql: &str,
    yaml: &str,
    yaml_uri: &str,
    lineage: Option<&ModelLineage>,
) -> Option<CodeAction> {
    plain_name(model)?;
    let dialect: &Dialect = &ctx.dialect;
    let parsed = JinjaSqlParser.parse(sql, dialect).ok()?;
    let inputs: Vec<(Input, Vec<String>)> = model_inputs(&parsed)
        .into_iter()
        .map(|input| {
            let columns = input_columns(ctx, &input, lineage);
            (input, columns)
        })
        .collect();
    let name = fresh_name(model, &existing_names(yaml));
    let entry = unit_test_lines(&name, model, &inputs, &output_columns(details, lineage));
    let edit = insert_entry(yaml, &entry)?;
    Some(CodeAction {
        title: format!("Scaffold a unit test for '{model}'"),
        kind: Some("refactor"),
        diagnostics: None,
        edit: Some(WorkspaceEdit {
            document_changes: vec![DocumentChange::Edit(TextDocumentEdit::new(
                yaml_uri.to_string(),
                vec![edit],
            ))],
        }),
        command: None,
        is_preferred: None,
    })
}

/// Append `entry` to the top-level `unit_tests:` list, or start one at
/// the end of the file.
fn insert_entry(yaml: &str, entry: &[String]) -> Option<cogitator_lsp::TextEdit> {
    let lines: Vec<&str> = yaml.lines().collect();
    let header = lines
        .iter()
        .position(|l| indent(l) == 0 && l.starts_with("unit_tests:"));
    let (after, pad, prefix) = match header {
        Some(at) => {
            let value = lines[at]["unit_tests:".len()..].trim();
            if !value.is_empty() && !value.starts_with('#') {
                return None; // an inline list: left to the reader
            }
            let end = block_end(&lines, at, 0);
            let pad = (at + 1..=end)
                .find(|&i| is_content(lines[i]) && lines[i].trim_start().starts_with('-'))
                .map_or(2, |i| indent(lines[i]));
            (end, pad, String::new())
        }
        None => {
            let last = lines.iter().rposition(|l| is_content(l)).unwrap_or(0);
            (last, 2, "\nunit_tests:\n".to_string())
        }
    };
    let pad = " ".repeat(pad);
    let mut body = prefix;
    for line in entry {
        body.push_str(&format!("{pad}{line}\n"));
    }
    if yaml.is_empty() {
        return Some(cogitator_lsp::TextEdit {
            range: cogitator_lsp::Range::default(),
            new_text: body.trim_start_matches('\n').to_string(),
        });
    }
    Some(insert_after_line(yaml, after, body))
}

#[cfg(test)]
#[path = "unit_test_scaffold_tests.rs"]
mod tests;
