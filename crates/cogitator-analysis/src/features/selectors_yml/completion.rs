//! Completion of selector methods and their values in selectors.yml,
//! and of existing tag names wherever dbt reads tags.

use std::collections::BTreeSet;

use cogitator_lsp::{completion_kind, CompletionItem, Position};
use serde_json::Value;

use super::line_context::{selectors_slot, tag_slot, Slot};
use crate::selectors::{Definition, Method, Node, ResourceType, Universe, METHODS};

/// Completion at `pos`, or `None` when the cursor is in no selector or
/// tag slot (the caller then offers its own items).
pub fn selector_completion(
    uri: &str,
    text: &str,
    pos: Position,
    u: &Universe,
    defs: &[Definition],
) -> Option<Vec<CompletionItem>> {
    let (line, character) = (pos.line as usize, pos.character as usize);
    let slot = if super::is_selectors_file(uri) {
        selectors_slot(text, line, character)?
    } else if reads_tags(uri) {
        tag_slot(text, line, character)?
    } else {
        return None;
    };
    let items = match slot {
        Slot::Method { prefix } => methods(u, &prefix, false),
        Slot::Value { method, prefix } => values(u, defs, &method)
            .into_iter()
            .filter(|(v, _)| v.starts_with(&prefix))
            .map(|(v, detail)| item(&v, &detail, completion_kind::REFERENCE, &v))
            .collect(),
        Slot::Word { prefix } => {
            let mut out = methods(u, &prefix, true);
            out.extend(
                names(u, |n| n.resource_type.is_parsed_node())
                    .into_iter()
                    .filter(|v| v.starts_with(&prefix))
                    .map(|v| item(&v, "node (fqn)", completion_kind::REFERENCE, &v)),
            );
            out
        }
        Slot::Tag { prefix } => tags(u)
            .into_iter()
            .filter(|t| t.starts_with(&prefix))
            .map(|t| item(&t, "existing tag", completion_kind::VARIABLE, &t))
            .collect(),
    };
    Some(items)
}

/// Whether `pos` is in a slot [`selector_completion`] answers — cheap,
/// so the dispatcher can decide before it builds a universe.
pub fn selector_completion_applies(uri: &str, text: &str, pos: Position) -> bool {
    let (line, character) = (pos.line as usize, pos.character as usize);
    if super::is_selectors_file(uri) {
        selectors_slot(text, line, character).is_some()
    } else {
        reads_tags(uri) && tag_slot(text, line, character).is_some()
    }
}

/// Files where dbt reads `tags:`: dbt_project.yml and properties YAML.
fn reads_tags(uri: &str) -> bool {
    uri.ends_with("dbt_project.yml") || crate::yaml_properties::is_properties_file(uri)
}

fn item(label: &str, detail: &str, kind: i32, insert: &str) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        detail: detail.to_string(),
        documentation: String::new(),
        kind,
        insert_text: insert.to_string(),
        sort_text: label.to_string(),
        data: None,
    }
}

/// The methods, with `config.<key>` for each config key in use; in a
/// string the method is inserted with its `:`.
fn methods(u: &Universe, prefix: &str, colon: bool) -> Vec<CompletionItem> {
    let keys: BTreeSet<&String> = u.nodes.iter().flat_map(|n| n.config.keys()).collect();
    let configs = keys.into_iter().map(|k| {
        (
            format!("config.{k}"),
            format!("nodes whose resolved `{k}` config matches"),
        )
    });
    METHODS
        .iter()
        .filter(|(name, _)| *name != "config")
        .map(|(name, doc)| ((*name).to_string(), (*doc).to_string()))
        .chain(configs)
        .filter(|(name, _)| name.starts_with(prefix))
        .map(|(name, doc)| {
            let insert = if colon {
                format!("{name}:")
            } else {
                name.clone()
            };
            item(&name, &doc, completion_kind::KEYWORD, &insert)
        })
        .collect()
}

/// Every value a method can take in this project, with a detail.
fn values(u: &Universe, defs: &[Definition], method: &Method) -> Vec<(String, String)> {
    let fixed = |vals: &[&str], detail: &str| -> Vec<(String, String)> {
        vals.iter()
            .map(|v| (v.to_string(), detail.to_string()))
            .collect()
    };
    let of = |vals: BTreeSet<String>, detail: &str| -> Vec<(String, String)> {
        vals.into_iter().map(|v| (v, detail.to_string())).collect()
    };
    match method {
        Method::Tag => of(tags(u), "tag"),
        Method::Fqn => of(names(u, |n| n.resource_type.is_parsed_node()), "node"),
        Method::Path => of(paths(u), "path"),
        Method::File => of(
            u.nodes
                .iter()
                .filter_map(|n| n.path.rsplit('/').next().filter(|f| !f.is_empty()))
                .map(str::to_string)
                .collect(),
            "file",
        ),
        Method::Package => {
            let mut v: BTreeSet<String> = u.nodes.iter().map(|n| n.package.clone()).collect();
            v.insert("this".into());
            of(v, "package")
        }
        Method::Config(key) => of(config_values(u, key), &format!("config.{key}")),
        Method::ResourceType => of(
            ResourceType::ALL
                .iter()
                .map(|t| t.as_str().to_string())
                .collect(),
            "resource type",
        ),
        Method::TestType => fixed(&["generic", "singular", "unit"], "test type"),
        Method::TestName => of(
            u.nodes
                .iter()
                .filter_map(|n| n.test.as_ref()?.test_name.clone())
                .collect(),
            "generic test",
        ),
        Method::Source => {
            let mut v = BTreeSet::new();
            for (s, t) in u.nodes.iter().filter_map(|n| n.source.as_ref()) {
                v.insert(s.clone());
                v.insert(format!("{s}.{t}"));
            }
            of(v, "source")
        }
        Method::Exposure => of(
            names(u, |n| n.resource_type == ResourceType::Exposure),
            "exposure",
        ),
        Method::Metric => of(
            names(u, |n| n.resource_type == ResourceType::Metric),
            "metric",
        ),
        Method::SemanticModel => of(
            names(u, |n| n.resource_type == ResourceType::SemanticModel),
            "semantic model",
        ),
        Method::UnitTest => of(
            names(u, |n| n.resource_type == ResourceType::UnitTest),
            "unit test",
        ),
        Method::Group => of(
            u.nodes.iter().filter_map(|n| n.group.clone()).collect(),
            "group",
        ),
        Method::Access => fixed(&["public", "protected", "private"], "access"),
        Method::Version => fixed(&["latest", "prerelease", "old", "none"], "version"),
        Method::Selector => of(defs.iter().map(|d| d.name.clone()).collect(), "selector"),
        Method::NeedsState("state") => fixed(
            &[
                "new",
                "modified",
                "modified.body",
                "modified.configs",
                "modified.relation",
                "modified.persisted_descriptions",
                "modified.macros",
                "modified.contract",
                "unmodified",
                "old",
            ],
            "state (needs --state)",
        ),
        Method::NeedsState("result") => fixed(
            &["error", "fail", "warn", "success", "skipped"],
            "result (needs --state)",
        ),
        Method::NeedsState(_) => fixed(
            &["fresher", "error", "warn", "pass"],
            "source status (needs --state)",
        ),
        Method::Wildcard => fixed(&["*"], "every node"),
        Method::SavedQuery | Method::Unknown(_) => Vec::new(),
    }
}

fn names(u: &Universe, keep: impl Fn(&Node) -> bool) -> BTreeSet<String> {
    u.nodes
        .iter()
        .filter(|n| keep(n))
        .map(|n| n.name.clone())
        .collect()
}

fn tags(u: &Universe) -> BTreeSet<String> {
    u.nodes
        .iter()
        .flat_map(|n| n.tags.iter().cloned())
        .collect()
}

/// Every file path and every directory above one.
fn paths(u: &Universe) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for n in u.nodes.iter().filter(|n| !n.path.is_empty()) {
        let mut p = n.path.as_str();
        out.insert(p.to_string());
        while let Some(i) = p.rfind('/') {
            p = &p[..i];
            out.insert(p.to_string());
        }
    }
    out
}

fn config_values(u: &Universe, key: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for n in &u.nodes {
        let mut parts = key.split('.');
        let mut current = parts.next().and_then(|k| n.config.get(k));
        for part in parts {
            current = current.and_then(|v| v.get(part));
        }
        match current {
            Some(Value::Array(items)) => out.extend(items.iter().filter_map(scalar)),
            Some(v) => out.extend(scalar(v)),
            None => {}
        }
    }
    out
}

fn scalar(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

#[cfg(test)]
#[path = "completion_tests.rs"]
mod tests;
