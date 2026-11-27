//! A model's card (`dbt.modelInfo`, `dbt.modelGraph`) and a node's
//! (`dbt.nodeInfo`): Neovim's `explorer/card.lua`, `view` and `node`, as
//! markdown. A section the answer has no field for is left out (the model
//! graph has no tests); one it has empty says so.

use serde_json::Value;

use super::{
    columns_table, description, escape, file_link, finished, heading, link, list, tags, text,
    NOT_INFERRED,
};

/// `dbt.modelInfo`'s answer (or `dbt.modelGraph`'s) as a card.
pub fn model(answer: &Value, root: Option<&str>) -> Option<String> {
    let name = text(answer, "model").or_else(|| text(answer, "name"))?;
    let uri = text(answer, "uri");
    let mut out = format!("# {}\n\n", escape(name));
    out.push_str("model");
    if let Some(uri) = uri {
        out.push_str(" · ");
        out.push_str(&file_link(uri, root));
    }
    out.push('\n');
    description(&mut out, answer);
    tags(&mut out, list(answer, "tags"));
    if answer.get("compiledUri").is_some() {
        files(&mut out, answer, root);
    }
    versions(&mut out, answer, root);
    neighbours(&mut out, "Upstream", list(answer, "upstream"), "nothing");
    neighbours(
        &mut out,
        "Downstream",
        list(answer, "downstream"),
        "nothing depends on it",
    );
    if answer.get("coverage").is_some() || answer.get("columnTypes").is_some() {
        columns(&mut out, answer);
    }
    if answer.get("tests").is_some() {
        tests(&mut out, list(answer, "tests"), root);
    }
    Some(finished(out))
}

fn files(out: &mut String, answer: &Value, root: Option<&str>) {
    out.push_str("\n## Files\n\n");
    match text(answer, "uri") {
        Some(uri) => out.push_str(&format!("- dbt SQL: {}\n", file_link(uri, root))),
        None => out.push_str("- dbt SQL: _no file_\n"),
    }
    match text(answer, "compiledUri") {
        Some(uri) => out.push_str(&format!("- compiled SQL: {}\n", file_link(uri, root))),
        None => out.push_str(
            "- compiled SQL: _not compiled yet — \"Compile it\" in the dbt menu writes it_\n",
        ),
    }
}

fn versions(out: &mut String, answer: &Value, root: Option<&str>) {
    let Some(v) = answer.get("versions").filter(|v| v.is_object()) else {
        return;
    };
    let latest = v
        .get("latestVersion")
        .and_then(Value::as_i64)
        .map_or("?".to_string(), |l| l.to_string());
    out.push_str(&format!("\n## Versions (latest {latest})\n\n"));
    for item in list(v, "versions") {
        let n = item.get("v").and_then(Value::as_i64).unwrap_or(0);
        let file = text(item, "uri").map_or("_no file_".to_string(), |u| file_link(u, root));
        out.push_str(&format!("- v{n} → {file}\n"));
    }
}

/// A neighbour on its line: its name linked to its file, and its kind.
fn entry(e: &Value) -> String {
    let name = text(e, "name").unwrap_or("?");
    match text(e, "kind") {
        Some(kind) => format!("{} · {kind}", link(name, text(e, "uri"))),
        None => link(name, text(e, "uri")),
    }
}

fn neighbours(out: &mut String, title: &str, entries: &[Value], empty: &str) {
    heading(out, title, entries.len());
    if entries.is_empty() {
        out.push_str(&format!("_{empty}_\n"));
    }
    for e in entries {
        out.push_str(&format!("- {}\n", entry(e)));
    }
}

fn columns(out: &mut String, answer: &Value) {
    let typed = answer
        .get("columnTypes")
        .map_or(&[][..], |t| list(t, "columns"));
    let coverage = answer.get("coverage");
    let count = coverage
        .and_then(|c| c.get("columns"))
        .and_then(Value::as_u64)
        .map_or(typed.len(), |n| (n as usize).max(typed.len()));
    heading(out, "Columns", count);
    if let Some(c) = coverage {
        let n = |key: &str| c.get(key).and_then(Value::as_u64).unwrap_or(0);
        out.push_str(&format!(
            "{} of {} documented · {} tested{}\n\n",
            n("documented"),
            n("columns"),
            n("tested"),
            if c.get("described").and_then(Value::as_bool) == Some(false) {
                " · the model has no description"
            } else {
                ""
            }
        ));
    }
    if typed.is_empty() {
        out.push_str(&format!("_{NOT_INFERRED}_\n"));
    } else {
        columns_table(out, typed);
    }
}

/// A test as Neovim labels it: `name on column (vN)`.
fn test_label(t: &Value) -> String {
    let mut label = text(t, "name").unwrap_or("?").to_string();
    if let Some(column) = text(t, "column") {
        label.push_str(&format!(" on {column}"));
    }
    if let Some(v) = t.get("version").and_then(Value::as_i64) {
        label.push_str(&format!(" (v{v})"));
    }
    label
}

/// The 1-based line a `range` starts on.
fn line_of(value: &Value) -> Option<u64> {
    value
        .get("range")
        .and_then(|r| r.get("start"))
        .and_then(|s| s.get("line"))
        .and_then(Value::as_u64)
        .map(|l| l + 1)
}

fn tests(out: &mut String, tests: &[Value], root: Option<&str>) {
    heading(out, "Tests", tests.len());
    if tests.is_empty() {
        out.push_str("_none_\n");
    }
    for t in tests {
        let unit = if t.get("unit").and_then(Value::as_bool) == Some(true) {
            " · unit test"
        } else {
            ""
        };
        let at = match (text(t, "uri"), line_of(t)) {
            (Some(uri), Some(line)) => format!(" — {} line {line}", super::path_of(uri, root)),
            _ => String::new(),
        };
        out.push_str(&format!(
            "- {}{unit}{}\n",
            link(&test_label(t), text(t, "uri")),
            escape(&at)
        ));
    }
}

/// A macro's or a generic test's signature: `name(a, b='x')`.
fn signature(name: &str, params: &[Value]) -> String {
    let parts: Vec<String> = params
        .iter()
        .map(|p| match p {
            Value::String(s) => s.clone(),
            p => {
                let name = text(p, "name").unwrap_or("?");
                match text(p, "default") {
                    Some(d) => format!("{name}={d}"),
                    None => name.to_string(),
                }
            }
        })
        .collect();
    format!("{name}({})", parts.join(", "))
}

/// `dbt.nodeInfo`'s answer as a card: a model's as [`model`], any other
/// node's as `card.lua`'s `node`.
pub fn node(answer: &Value, root: Option<&str>) -> Option<String> {
    let kind = text(answer, "kind")?;
    if kind == "model" {
        return model(answer, root);
    }
    let name = text(answer, "name")?;
    let mut out = format!("# {}\n\n{kind}", escape(name));
    if let Some(uri) = text(answer, "uri") {
        out.push_str(" · ");
        out.push_str(&file_link(uri, root));
    }
    out.push('\n');
    description(&mut out, answer);
    let package = |out: &mut String| {
        if let Some(p) = text(answer, "package") {
            out.push_str(&format!("\npackage: {}\n", escape(p)));
        }
    };
    match kind {
        "macro" => {
            out.push_str(&format!(
                "\n`{}`\n",
                signature(name, list(answer, "params"))
            ));
            package(&mut out);
            neighbours(
                &mut out,
                "Called by",
                list(answer, "callers"),
                "no model or test calls it",
            );
            neighbours(&mut out, "Calls", list(answer, "calls"), "no other macro");
        }
        "generic_test" => {
            out.push_str(&format!(
                "\n`{{% test {} %}}`\n",
                signature(name, list(answer, "params"))
            ));
            package(&mut out);
            let applied = list(answer, "appliedTo");
            heading(&mut out, "Applied to", applied.len());
            if applied.is_empty() {
                out.push_str("_no YAML applies it_\n");
            }
            for a in applied {
                let mut target = text(a, "model").unwrap_or("?").to_string();
                if let Some(column) = text(a, "column") {
                    target.push('.');
                    target.push_str(column);
                }
                let line = line_of(a)
                    .map(|l| format!(" — line {l}"))
                    .unwrap_or_default();
                out.push_str(&format!("- {}{line}\n", link(&target, text(a, "uri"))));
            }
        }
        _ => {
            let test_like = kind == "test" || kind == "analysis";
            let refs = if answer.get("refs").is_some() {
                list(answer, "refs")
            } else {
                list(answer, "upstream")
            };
            neighbours(
                &mut out,
                if kind == "test" { "Tests" } else { "Upstream" },
                refs,
                "nothing",
            );
            if !list(answer, "calls").is_empty() {
                neighbours(&mut out, "Calls", list(answer, "calls"), "no macro");
            }
            if test_like {
                out.push_str("\n## Compiled\n\n");
                match text(answer, "compiledUri") {
                    Some(uri) => out.push_str(&format!("{}\n", file_link(uri, root))),
                    None => out.push_str("_not yet (dbt compile)_\n"),
                }
            } else {
                neighbours(
                    &mut out,
                    "Downstream",
                    list(answer, "downstream"),
                    "nothing depends on it",
                );
            }
        }
    }
    Some(finished(out))
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
