//! `dbt.modelDependencies` as a card: Neovim's dependencies dialog
//! (`deps.lua`: `card`, `columns_lines`, `items`) as markdown — the model,
//! how it is built, its columns, then what it uses and what uses it,
//! directly and everything, nearest first.

use serde_json::Value;

use super::{
    columns_table, escape, file_link, finished, heading, link, list, tags, text, NOT_INFERRED,
};

/// The config's strings of `key`: one or several.
fn strings<'a>(config: &'a Value, key: &str) -> Vec<&'a str> {
    list(config, key).iter().filter_map(Value::as_str).collect()
}

/// A node's config on one line: `incremental · merge · unique_key id, day ·
/// partition day` (`deps.lua`'s `summary`).
pub fn summary(config: &Value) -> String {
    let mut parts: Vec<String> = Vec::new();
    for key in ["materialized", "incrementalStrategy"] {
        if let Some(v) = text(config, key) {
            parts.push(v.to_string());
        }
    }
    for (key, label) in [
        ("uniqueKey", "unique_key"),
        ("partitionBy", "partition"),
        ("clusterBy", "cluster"),
    ] {
        let values = strings(config, key);
        if !values.is_empty() {
            parts.push(format!("{label} {}", values.join(", ")));
        }
    }
    parts.join(" · ")
}

/// How the model is built, a row per part of its config that is set.
fn build(out: &mut String, config: &Value) {
    let mut rows: Vec<(&str, String)> = Vec::new();
    for (label, key) in [
        ("materialized", "materialized"),
        ("incremental_strategy", "incrementalStrategy"),
    ] {
        if let Some(v) = text(config, key) {
            rows.push((label, v.to_string()));
        }
    }
    for (label, key) in [
        ("unique_key", "uniqueKey"),
        ("partition_by", "partitionBy"),
        ("cluster_by", "clusterBy"),
    ] {
        let values = strings(config, key);
        if !values.is_empty() {
            rows.push((label, values.join(", ")));
        }
    }
    if let Some(v) = text(config, "onSchemaChange") {
        rows.push(("on_schema_change", v.to_string()));
    }
    out.push_str("\n## How it is built\n\n");
    if rows.is_empty() {
        out.push_str("_no config set: dbt's defaults_\n");
    } else {
        out.push_str("| config | value |\n|---|---|\n");
        for (label, value) in rows {
            out.push_str(&format!("| {label} | `{}` |\n", value.replace('|', "\\|")));
        }
    }
    if text(config, "materialized") == Some("incremental")
        && strings(config, "uniqueKey").is_empty()
    {
        out.push_str("\n> No `unique_key`: every run appends.\n");
    }
}

/// A model's columns with their types (`deps.lua`'s `columns_lines`).
fn columns(out: &mut String, node: &Value) {
    match node.get("columns").and_then(Value::as_array) {
        None => {
            out.push_str("\n## Columns\n\n");
            out.push_str(&format!("_{NOT_INFERRED}_\n"));
        }
        Some(columns) if columns.is_empty() => {
            heading(out, "Columns", 0);
            out.push_str("_no columns_\n");
        }
        Some(columns) => {
            heading(out, "Columns", columns.len());
            columns_table(out, columns);
        }
    }
}

/// A node on its line: an arrow and its hops for a walk, its name linked
/// to its file (and line), its kind and how it is built.
fn item(node: &Value, arrow: Option<&str>, root: Option<&str>) -> String {
    let name = text(node, "name").unwrap_or("?");
    let mut line = String::new();
    if let Some(arrow) = arrow {
        let depth = node.get("depth").and_then(Value::as_u64).unwrap_or(0);
        line.push_str(&format!("{arrow}{depth} "));
    }
    line.push_str(&link(name, text(node, "uri")));
    if let Some(kind) = text(node, "kind") {
        line.push_str(&format!(" · {kind}"));
    }
    if let Some(package) = text(node, "package") {
        line.push_str(&format!(" of {}", escape(package)));
    }
    let summary = node.get("config").map(summary).unwrap_or_default();
    if !summary.is_empty() {
        line.push_str(&format!(" · `{summary}`"));
    }
    match (text(node, "uri"), node.get("line").and_then(Value::as_u64)) {
        (Some(uri), Some(l)) if l > 0 => line.push_str(&format!(
            " — {} line {}",
            escape(&super::path_of(uri, root)),
            l + 1
        )),
        _ if text(node, "uri").is_none() => line.push_str(" — _no file on disk_"),
        _ => {}
    }
    line
}

fn group(out: &mut String, title: &str, nodes: &[Value], arrow: Option<&str>, root: Option<&str>) {
    heading(out, title, nodes.len());
    if nodes.is_empty() {
        out.push_str("_nothing_\n");
    }
    for node in nodes {
        out.push_str(&format!("- {}\n", item(node, arrow, root)));
    }
}

/// `dbt.modelDependencies`' answer as a card.
pub fn card(answer: &Value, root: Option<&str>) -> Option<String> {
    let model = answer.get("model").filter(|m| m.is_object())?;
    let name = text(model, "name")?;
    let mut out = format!("# {}\n\n", escape(name));
    out.push_str(text(model, "kind").unwrap_or("model"));
    if let Some(package) = text(model, "package") {
        out.push_str(&format!(" of {}", escape(package)));
    }
    match text(model, "uri") {
        Some(uri) => out.push_str(&format!(" · {}", file_link(uri, root))),
        None => out.push_str(" · _no file on disk_"),
    }
    out.push('\n');
    let config = model.get("config").cloned().unwrap_or(Value::Null);
    build(&mut out, &config);
    tags(&mut out, list(model, "tags"));
    if text(model, "kind").is_none_or(|k| k == "model") {
        columns(&mut out, model);
    }
    group(
        &mut out,
        "Uses directly",
        list(answer, "parents"),
        None,
        root,
    );
    group(
        &mut out,
        "Used directly by",
        list(answer, "children"),
        None,
        root,
    );
    group(
        &mut out,
        "Everything upstream",
        list(answer, "upstream"),
        Some("↑"),
        root,
    );
    group(
        &mut out,
        "Everything downstream",
        list(answer, "downstream"),
        Some("↓"),
        root,
    );
    Some(finished(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn a_config_is_summed_up_on_one_line() {
        let cases = [
            (json!({}), ""),
            (json!({"materialized": "view"}), "view"),
            (
                json!({"materialized": "incremental", "incrementalStrategy": "merge",
                       "uniqueKey": ["id", "day"], "partitionBy": ["day"], "clusterBy": []}),
                "incremental · merge · unique_key id, day · partition day",
            ),
            (json!({"clusterBy": ["a"]}), "cluster a"),
        ];
        for (config, shown) in cases {
            assert_eq!(summary(&config), shown, "{config}");
        }
    }

    fn node(name: &str, depth: u64, config: Value) -> Value {
        json!({
            "name": name, "kind": "model", "package": "", "uri": format!("file:///p/models/{name}.sql"),
            "line": 0, "depth": depth, "tags": [], "config": config, "columns": null,
        })
    }

    #[test]
    fn the_dependencies_card() {
        let answer = json!({
            "model": {
                "name": "orders", "kind": "model", "package": "", "uri": "file:///p/models/orders.sql",
                "line": 0, "depth": 0, "tags": ["nightly"],
                "config": {"materialized": "incremental", "uniqueKey": [], "partitionBy": [], "clusterBy": []},
                "columns": [
                    {"name": "order_id", "dataType": "INTEGER", "description": "INTEGER", "nullable": false, "basis": "proven", "why": "x"},
                    {"name": "status", "dataType": null, "description": "text", "nullable": true, "basis": "declared", "why": "y"}
                ]
            },
            "parents": [node("stg_orders", 1, json!({"materialized": "view"}))],
            "children": [],
            "upstream": [
                node("stg_orders", 1, json!({"materialized": "view"})),
                {"name": "raw.orders", "kind": "source", "package": "", "uri": "file:///p/models/sources.yml",
                 "line": 6, "depth": 2, "tags": [], "config": {}, "columns": null}
            ],
            "downstream": [],
        });
        insta::assert_snapshot!(card(&answer, Some("file:///p")).unwrap());
    }

    #[test]
    fn columns_say_what_is_not_known() {
        let mut out = String::new();
        columns(&mut out, &json!({"columns": null}));
        assert!(out.contains(NOT_INFERRED), "{out}");
        let mut out = String::new();
        columns(&mut out, &json!({"columns": []}));
        assert!(
            out.contains("## Columns (0)") && out.contains("no columns"),
            "{out}"
        );
    }

    #[test]
    fn no_model_no_card() {
        assert_eq!(card(&json!({"model": null}), None), None);
    }
}
