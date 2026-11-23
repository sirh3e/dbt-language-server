//! dbt node selection: `--select` / `--exclude` text over the project's
//! nodes, answered as dbt answers it (dbt.select), and what completing a
//! selection offers (dbt.selectionVocabulary). The parser, the universe
//! of nodes and the evaluator are pure; the universe is built from the
//! context once and cached by the server.

mod eval;
mod folder_config;
mod glob;
pub mod parse;
pub mod universe;

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

pub use parse::SelectionError;
pub use universe::{build as build_universe, Node, Universe};

use crate::DbtContext;

/// The dbt.select answer: the nodes in a stable order (by resource type,
/// then name), how many of each type, or why the text is no selection.
#[derive(Debug, Default, Serialize)]
pub struct SelectionResult {
    pub nodes: Vec<Node>,
    pub counts: BTreeMap<&'static str, usize>,
    pub error: Option<SelectionError>,
}

/// Evaluate `select` (which may carry its own `--exclude …`) minus
/// `exclude`. Error spans are byte offsets into `select` (or into
/// `exclude` when that is where the error is).
pub fn select(
    ctx: &DbtContext,
    universe: &Universe,
    select: &str,
    exclude: Option<&str>,
) -> SelectionResult {
    let (text, inline) = parse::split_exclude(select);
    // offset of the inline exclusion inside `select`
    let inline_at = inline.map(|ex| select.len() - ex.len());
    let failed = |mut error: SelectionError, offset: usize| {
        error.start += offset;
        error.end += offset;
        SelectionResult {
            error: Some(error),
            ..Default::default()
        }
    };
    let evaluator = eval::Eval {
        universe,
        selectors: &ctx.selector_detail_map,
        root_package: &ctx.project.name,
    };
    // each part parsed and evaluated on its own, so an error points into
    // the text it is in
    let picked = match parse::parse(text).and_then(|s| evaluator.select(&s, None)) {
        Ok(set) => set,
        Err(e) => return failed(e, 0),
    };
    let mut picked = picked;
    for (part, offset) in [(inline, inline_at.unwrap_or(0)), (exclude, 0)] {
        let Some(part) = part.filter(|p| !p.trim().is_empty()) else {
            continue;
        };
        match parse::parse(part).and_then(|s| evaluator.excluded(&s)) {
            Ok(set) => picked.retain(|i| !set.contains(i)),
            Err(e) => return failed(e, offset),
        }
    }
    let mut nodes: Vec<Node> = picked
        .into_iter()
        .map(|i| universe.nodes[i].clone())
        .collect();
    let order = |t: &str| {
        eval::RESOURCE_TYPES
            .iter()
            .position(|r| *r == t)
            .unwrap_or(usize::MAX)
    };
    nodes.sort_by(|a, b| (order(a.resource_type), &a.name).cmp(&(order(b.resource_type), &b.name)));
    let mut counts = BTreeMap::new();
    for n in &nodes {
        *counts.entry(n.resource_type).or_insert(0) += 1;
    }
    SelectionResult {
        nodes,
        counts,
        error: None,
    }
}

/// What completing a selection offers: the methods, and the values each
/// method takes in this project.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Vocabulary {
    pub methods: Vec<&'static str>,
    pub values: BTreeMap<&'static str, Vec<String>>,
}

pub fn vocabulary(ctx: &DbtContext, universe: &Universe) -> Vocabulary {
    let mut values: BTreeMap<&'static str, BTreeSet<String>> = BTreeMap::new();
    let mut add = |method: &'static str, value: &str| {
        if !value.is_empty() {
            values.entry(method).or_default().insert(value.to_string());
        }
    };
    for n in &universe.nodes {
        for tag in &n.tags {
            add("tag", tag);
        }
        add("package", &n.package);
        add("resource_type", n.resource_type);
        if let Some(m) = &n.materialized {
            add("config.materialized", m);
        }
        if let Some((dir, _)) = n.path.rsplit_once('/') {
            add("path", dir);
        }
        if let Some(g) = &n.group {
            add("group", g);
        }
        if let Some(a) = &n.access {
            add("access", a);
        }
        match n.resource_type {
            "source" => {
                let source = n.source.as_deref().unwrap_or_default();
                add("source", source);
                add("source", &format!("{source}.{}", n.name));
            }
            "exposure" | "metric" | "semantic_model" => add(n.resource_type, &n.name),
            "unit_test" => add("unit_test", &n.name),
            _ => {}
        }
        if let Some(t) = &n.test_name {
            add("test_name", t.rsplit('.').next().unwrap_or(t));
        }
        if matches!(n.resource_type, "model" | "seed" | "snapshot") {
            add("fqn", &n.name);
        }
    }
    for name in ctx.selector_detail_map.keys() {
        add("selector", name);
    }
    for t in ["generic", "singular", "unit", "data"] {
        add("test_type", t);
    }
    add("package", "this");
    let mut methods: Vec<&'static str> = parse::METHODS.to_vec();
    methods.retain(|m| *m != "config" && *m != "version");
    methods.push("config.materialized");
    methods.push("config.tags");
    methods.sort_unstable();
    Vocabulary {
        methods,
        values: values
            .into_iter()
            .map(|(k, v)| (k, v.into_iter().collect()))
            .collect(),
    }
}
