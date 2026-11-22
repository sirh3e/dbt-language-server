//! `dbt.nodeInfo {node: {kind, name}}` (specs/highlighting/00-highlighting.md
//! §6.1): a card's data for any graph node — macros, singular and generic
//! tests, sources, exposures… — as `dbt.modelInfo` gives it for models.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use cogitator_analysis::context::DbtContext;
use cogitator_analysis::graph::{EdgeKind, NodeData, NodeKind};
use cogitator_lsp::error_codes;
use serde_json::{json, Value};

use crate::server::highlight::node_file;
use crate::server::reply::Reply;

/// A `{% test name(args) %}` definition: a generic test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericTest {
    pub name: String,
    pub params: Vec<String>,
    pub path: PathBuf,
    /// 0-based line of the `{% test %}` tag, and of its `{% endtest %}`.
    pub line: u32,
    pub end_line: u32,
}

/// The generic tests defined in `text` (a macro or test file).
pub fn generic_tests_in(text: &str, path: &Path) -> Vec<GenericTest> {
    let mut out = Vec::new();
    let mut search = 0;
    while let Some(open) = text[search..].find("{%").map(|i| i + search) {
        search = open + 2;
        let rest = text[open + 2..].trim_start_matches(['-', '+']).trim_start();
        let Some(after) = rest.strip_prefix("test") else {
            continue;
        };
        if !after.starts_with(char::is_whitespace) {
            continue;
        }
        let after = after.trim_start();
        let name: String = after
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if name.is_empty() {
            continue;
        }
        let params = after[name.len()..]
            .trim_start()
            .strip_prefix('(')
            .and_then(|p| p.split_once(')'))
            .map(|(inside, _)| {
                inside
                    .split(',')
                    .map(|p| p.split('=').next().unwrap_or("").trim().to_string())
                    .filter(|p| !p.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        let line = text[..open].matches('\n').count() as u32;
        let end_line = text[open..]
            .find("endtest")
            .map(|i| text[..open + i].matches('\n').count() as u32)
            .unwrap_or(line);
        out.push(GenericTest {
            name,
            params,
            path: path.to_path_buf(),
            line,
            end_line,
        });
    }
    out
}

/// The `.sql` files below `dir`.
fn sql_files(dir: &Path, out: &mut BTreeSet<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            sql_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("sql") {
            out.insert(path);
        }
    }
}

/// Every generic test of the project and its packages: the project's
/// macro and test paths, and every macro file the context knows.
pub fn generic_tests(ctx: &DbtContext) -> Vec<GenericTest> {
    let mut files = BTreeSet::new();
    if let Some(root) = ctx.project_root.as_deref() {
        for dir in ctx
            .project
            .macro_paths
            .iter()
            .chain(&ctx.project.test_paths)
        {
            sql_files(&root.join(dir), &mut files);
        }
    }
    for macros in ctx.macro_detail_map.values() {
        for m in macros.values() {
            files.insert(m.uri.clone());
        }
    }
    files
        .iter()
        .filter_map(|p| {
            std::fs::read_to_string(p)
                .ok()
                .map(|t| generic_tests_in(&t, p))
        })
        .flatten()
        .collect()
}

fn uri_of(path: &Path) -> Value {
    if path.as_os_str().is_empty() {
        Value::Null
    } else {
        json!(cogitator_lsp::fs_path_to_uri(path))
    }
}

/// A neighbour as the card lists it, its file found when the graph has
/// none (a macro is a node before it is a file).
fn entry(ctx: &DbtContext, node: &NodeData) -> Value {
    let path = if node.uri.as_os_str().is_empty() {
        node_file(ctx, node.kind.as_str(), &node.name).unwrap_or_default()
    } else {
        node.uri.clone()
    };
    json!({"kind": node.kind.as_str(), "name": node.name, "uri": uri_of(&path)})
}

fn range(line: u32, end_line: u32) -> Value {
    json!({"start": {"line": line, "character": 0}, "end": {"line": end_line, "character": 0}})
}

/// The macros a macro calls: every other known macro whose name is called
/// in the macro's body.
fn macro_calls(ctx: &DbtContext, own: &str, body: &str) -> Vec<Value> {
    let mut found = BTreeSet::new();
    for macros in ctx.macro_detail_map.values() {
        for (name, details) in macros {
            if name == own || found.contains(&(name.clone(), details.uri.clone())) {
                continue;
            }
            let called = body.match_indices(name.as_str()).any(|(i, _)| {
                let before = body[..i].chars().next_back();
                let after = body[i + name.len()..].trim_start().starts_with('(');
                after && !before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
            });
            if called {
                found.insert((name.clone(), details.uri.clone()));
            }
        }
    }
    found
        .into_iter()
        .map(|(name, path)| json!({"kind": "macro", "name": name, "uri": uri_of(&path)}))
        .collect()
}

fn unknown(kind: &str, name: &str, ctx: &DbtContext, quiet: bool) -> Reply {
    if quiet {
        return Reply::null();
    }
    let names: Vec<String> = match kind {
        "macro" => ctx
            .macro_detail_map
            .values()
            .flat_map(|m| m.keys().cloned())
            .collect(),
        "generic_test" => generic_tests(ctx).into_iter().map(|t| t.name).collect(),
        "model" => ctx.model_detail_map.keys().cloned().collect(),
        _ => Vec::new(),
    };
    let guidance = cogitator_error::guidance::Guidance::new(format!("No {kind} '{name}' in this project."))
        .suggest(cogitator_error::guidance::closest_matches(name, names.iter().map(String::as_str)))
        .fix("pass {\"node\": {\"kind\": \"model|macro|test|generic_test|source|exposure|metric|semantic_model|analysis\", \"name\": …}}.");
    Reply::null().with(crate::server::reply::Effect::ShowMessage {
        level: cogitator_lsp::message_type::WARNING,
        text: guidance.to_message(),
    })
}

/// `dbt.nodeInfo`. `quiet` leaves out the WARNING for an unknown node (a
/// client trying kinds in turn).
pub fn reply(ctx: &DbtContext, kind: &str, name: &str, quiet: bool) -> Reply {
    if name.is_empty() {
        return Reply::error(
            error_codes::INVALID_PARAMS,
            cogitator_error::guidance::Guidance::new("dbt.nodeInfo needs a node.")
                .fix("pass {\"node\": {\"kind\": \"macro\", \"name\": \"cents_to_dollars\"}}.")
                .to_message(),
        );
    }
    let root = ctx.project_root.as_deref();
    match kind {
        "model" => match cogitator_analysis::features::model_info(ctx, "", Some(name)) {
            Some(info) => {
                let mut value = serde_json::to_value(info).unwrap_or(Value::Null);
                value["kind"] = json!("model");
                Reply::result(&value)
            }
            None => unknown(kind, name, ctx, quiet),
        },
        "macro" => {
            let Some((package, details)) = ctx
                .macro_detail_map
                .values()
                .find_map(|m| m.get(name).map(|d| (d.project_name.clone(), d)))
            else {
                return unknown(kind, name, ctx, quiet);
            };
            let text = std::fs::read_to_string(&details.uri).unwrap_or_default();
            let lines: Vec<&str> = text.lines().collect();
            let (start, end) = (
                details.range.start.line as usize,
                details.range.end.line as usize,
            );
            let body = lines
                .get(start..=end.min(lines.len().saturating_sub(1)).max(start))
                .map(|l| l.join("\n"))
                .unwrap_or_default();
            let (_, incoming) = ctx.graph.node_edges(NodeKind::Macro, name);
            let callers: Vec<Value> = incoming
                .iter()
                .filter(|(edge, _)| matches!(edge, EdgeKind::MacroCall))
                .map(|(_, n)| entry(ctx, n))
                .collect();
            let used_by_tests: Vec<Value> = callers
                .iter()
                .filter(|c| c["kind"] == "test")
                .cloned()
                .collect();
            Reply::result(&json!({
                "kind": "macro",
                "name": name,
                "package": package,
                "description": details.description,
                "params": details.params.iter().map(|p| json!({"name": p.name, "default": p.default})).collect::<Vec<_>>(),
                "uri": uri_of(&details.uri),
                "range": details.range,
                "callers": callers,
                "calls": macro_calls(ctx, name, &body),
                "usedByTests": used_by_tests,
            }))
        }
        "generic_test" => {
            let tests = generic_tests(ctx);
            let Some(test) = tests.iter().find(|t| t.name == name) else {
                return unknown(kind, name, ctx, quiet);
            };
            let package = ctx
                .macro_detail_map
                .values()
                .flat_map(|m| m.values())
                .find(|m| m.uri == test.path)
                .map(|m| m.project_name.clone())
                .unwrap_or_else(|| ctx.project.name.clone());
            let mut applied = Vec::new();
            let mut models: Vec<&String> = ctx.model_detail_map.keys().collect();
            models.sort();
            for model in models {
                let uri = cogitator_lsp::fs_path_to_uri(&ctx.model_detail_map[model].uri);
                let Some((_, model_tests)) = cogitator_analysis::features::model_tests(ctx, &uri)
                else {
                    continue;
                };
                for t in model_tests {
                    let short = t.name.rsplit('.').next().unwrap_or(&t.name);
                    if !t.unit && short == name {
                        applied.push(json!({
                            "model": model, "column": t.column, "uri": t.uri, "range": t.range,
                        }));
                    }
                }
            }
            Reply::result(&json!({
                "kind": "generic_test",
                "name": name,
                "package": package,
                "params": test.params,
                "uri": uri_of(&test.path),
                "range": range(test.line, test.end_line),
                "appliedTo": applied,
            }))
        }
        other => {
            let Some(node_kind) = NodeKind::parse(other) else {
                return Reply::error(
                    error_codes::INVALID_PARAMS,
                    cogitator_error::guidance::Guidance::new(format!("dbt.nodeInfo does not know the kind '{other}'."))
                        .fix("one of model, macro, test, generic_test, source, exposure, metric, semantic_model, analysis.")
                        .to_message(),
                );
            };
            let Some(node) = ctx.graph.node(node_kind, name) else {
                return unknown(kind, name, ctx, quiet);
            };
            let (outgoing, incoming) = ctx.graph.node_edges(node_kind, name);
            let refs: Vec<Value> = outgoing
                .iter()
                .filter(|(e, _)| !matches!(e, EdgeKind::MacroCall))
                .map(|(_, n)| entry(ctx, n))
                .collect();
            let calls: Vec<Value> = outgoing
                .iter()
                .filter(|(e, _)| matches!(e, EdgeKind::MacroCall))
                .map(|(_, n)| entry(ctx, n))
                .collect();
            let downstream: Vec<Value> = incoming.iter().map(|(_, n)| entry(ctx, n)).collect();
            let path = node_file(ctx, other, name).unwrap_or_else(|| node.uri.clone());
            let compiled = (node_kind == NodeKind::Test || node_kind == NodeKind::Analysis)
                .then(|| {
                    root.map(|r| {
                        cogitator_analysis::features::compiled_sql_path(r, &ctx.project.name, &path)
                    })
                })
                .flatten()
                .filter(|p| p.is_file());
            let description = match node_kind {
                NodeKind::Source => ctx
                    .source_detail_map
                    .get(name)
                    .map(|s| s.description.clone()),
                _ => None,
            };
            Reply::result(&json!({
                "kind": other,
                "name": name,
                "uri": uri_of(&path),
                "description": description,
                "refs": refs,
                "calls": calls,
                "upstream": refs,
                "downstream": downstream,
                "compiledUri": compiled.as_deref().map(cogitator_lsp::fs_path_to_uri),
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_tests_are_found_with_their_params_and_lines() {
        let text = "{% macro x() %}{% endmacro %}\n\n{%- test not_negative(model, column_name, strict=true) -%}\nselect 1\n{%- endtest %}\n{% test   other() %}{% endtest %}";
        let found = generic_tests_in(text, Path::new("/p/macros/t.sql"));
        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(found[0].name, "not_negative");
        assert_eq!(found[0].params, vec!["model", "column_name", "strict"]);
        assert_eq!((found[0].line, found[0].end_line), (2, 4));
        assert_eq!(found[1].name, "other");
        assert!(found[1].params.is_empty());
    }

    #[test]
    fn a_tests_word_is_no_test_block() {
        assert!(
            generic_tests_in("{% set tests = 1 %}{% testing() %}", Path::new("a.sql")).is_empty()
        );
    }
}
