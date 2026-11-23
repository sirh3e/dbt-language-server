//! `dbt.select`: the nodes a selection picks, as `dbt ls --select …
//! --exclude …` lists them (eager indirect selection), with where each is
//! defined, how many of each type, what is wrong with the text and where,
//! and (when asked) what completing a selection offers.

use std::collections::{BTreeMap, BTreeSet};

use cogitator_lsp::Range;
use serde::Serialize;

use crate::selectors::{
    self, Criterion, Definition, Expr, Indirect, Method, Node, ResourceType, Selection, Universe,
    METHODS,
};

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SelectedNode {
    /// The file to open; `None` for a node of no file of the project.
    pub uri: Option<String>,
    /// dbt's name: a model's without its version, a source's table.
    pub name: String,
    /// How `dbt ls` lists it: `orders.v1`, `landing.items`.
    pub label: String,
    pub kind: &'static str,
    pub resource_type: &'static str,
    /// dbt's unique id, without a test's hash suffix.
    pub unique_id: String,
    /// A versioned model's version (`1` of `orders.v1`).
    pub version: Option<String>,
    pub package: String,
    /// The defining file relative to its package, `/`-separated.
    pub path: String,
    pub tags: Vec<String>,
    pub range: Range,
    /// The node's id in dbt.projectGraph, where it has one.
    pub graph_id: Option<String>,
}

/// Why a selection selects nothing: what, and where (byte offsets into
/// `select`, or into `exclude` when the error is there).
#[derive(Debug, Serialize, PartialEq, Clone)]
pub struct SelectError {
    pub message: String,
    pub start: usize,
    pub end: usize,
}

/// What completing a selection offers: the methods, and the values each
/// method takes in this project.
#[derive(Debug, Default, Serialize, PartialEq)]
pub struct Vocabulary {
    pub methods: Vec<String>,
    pub values: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct SelectAnswer {
    pub selector: String,
    pub count: usize,
    /// How many of each resource type.
    pub counts: BTreeMap<&'static str, usize>,
    pub nodes: Vec<SelectedNode>,
    /// What the selection asks that is not evaluated (unknown methods or
    /// selectors, cycles, state methods), in words.
    pub issues: Vec<String>,
    /// What dbt would refuse the selection for: an answer with an error
    /// lists no node, as dbt runs none.
    pub error: Option<SelectError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vocabulary: Option<Vocabulary>,
}

/// `select` may carry its own `--exclude …`; `exclude` is one more.
pub struct SelectRequest<'a> {
    pub select: &'a str,
    pub exclude: Option<&'a str>,
    pub vocabulary: bool,
}

/// `text` split at its first `--exclude`: (selection, exclusion).
pub fn split_exclude(text: &str) -> (&str, Option<&str>) {
    match text.find("--exclude") {
        Some(i) => (&text[..i], Some(&text[i + "--exclude".len()..])),
        None => (text, None),
    }
}

/// Evaluate a selection request.
pub fn select_nodes(u: &Universe, defs: &[Definition], req: &SelectRequest<'_>) -> SelectAnswer {
    let mut answer = SelectAnswer {
        selector: req.select.to_string(),
        count: 0,
        counts: BTreeMap::new(),
        nodes: Vec::new(),
        issues: Vec::new(),
        error: None,
        vocabulary: req.vocabulary.then(|| vocabulary(u, defs)),
    };
    let (text, inline) = split_exclude(req.select);
    let inline_at = req.select.len() - inline.map_or(0, str::len);
    let mut parts = vec![(text, 0)];
    parts.extend(inline.map(|e| (e, inline_at)));
    parts.extend(req.exclude.map(|e| (e, 0)));
    let mut selections = Vec::new();
    for (part, offset) in parts {
        if let Some(at) = part.find("--exclude") {
            answer.error = Some(SelectError {
                message: "a second `--exclude`: write one, with every exclusion after it".into(),
                start: offset + at,
                end: offset + at + "--exclude".len(),
            });
            return answer;
        }
        match selectors::parse(part) {
            Ok(selection) => {
                if let Some(error) = refused(&selection, defs, part, offset) {
                    // what is not evaluated, in words, as for any issue
                    answer.issues.push(error.message.clone());
                    answer.error = Some(error);
                    return answer;
                }
                selections.push(selection);
            }
            Err(e) => {
                answer.error = Some(SelectError {
                    message: e.message,
                    start: e.span.0 + offset,
                    end: e.span.1 + offset,
                });
                return answer;
            }
        }
    }
    let mut iter = selections.iter();
    let base = selectors::expr_of(iter.next().expect("the selection"));
    let expr = iter.fold(base, |acc, ex| {
        if ex.union.is_empty() {
            acc
        } else {
            Expr::Difference(Box::new(acc), Box::new(selectors::expr_of(ex)))
        }
    });
    let eval = selectors::select_expr(u, defs, &expr, Indirect::Eager);
    answer.issues = eval.issues.iter().map(selectors::issue_message).collect();
    if let Some(issue) = answer.issues.first() {
        // a cycle through selectors.yml: dbt refuses the whole selection
        answer.error = Some(SelectError {
            message: issue.clone(),
            start: 0,
            end: req.select.len(),
        });
        return answer;
    }
    answer.nodes = selectors::sorted(u, &eval)
        .into_iter()
        .map(|n| selected(n, &u.root_package))
        .collect();
    for n in &answer.nodes {
        *answer.counts.entry(n.resource_type).or_insert(0) += 1;
    }
    answer.count = answer.nodes.len();
    answer
}

/// `{selector}` alone, as the lenses and the run commands check one: a
/// selection dbt would refuse is an error in words.
pub fn select_command(
    u: &Universe,
    defs: &[Definition],
    selector: &str,
) -> Result<SelectAnswer, String> {
    let answer = select_nodes(
        u,
        defs,
        &SelectRequest {
            select: selector,
            exclude: None,
            vocabulary: false,
        },
    );
    match &answer.error {
        Some(e) => Err(format!("invalid selector: {}", e.message)),
        None => Ok(answer),
    }
}

/// What dbt refuses before evaluating: an unknown method, a resource type
/// or selector there is none of, a method that needs a previous run.
/// `part` is the text the selection was parsed from, at `offset`.
fn refused(
    selection: &Selection,
    defs: &[Definition],
    part: &str,
    offset: usize,
) -> Option<SelectError> {
    let at = |c: &Criterion, message: String| SelectError {
        message,
        start: c.span.0 + offset,
        end: c.span.1 + offset,
    };
    for c in selection.union.iter().flatten() {
        match &c.method {
            Method::Unknown(name) => {
                // the method as written, past a leading `@` or `N+`
                let written = format!("{name}:");
                let within = part[c.span.0..c.span.1].find(&written).unwrap_or(0);
                let start = c.span.0 + within + offset;
                return Some(SelectError {
                    // short, for a one-line prompt; completion lists them
                    message: format!("dbt has no selection method `{written}`"),
                    start,
                    end: start + written.len(),
                });
            }
            Method::NeedsState(method) => {
                return Some(at(
                    c,
                    format!("`{method}:` needs a previous dbt run's artifacts (dbt --state)"),
                ));
            }
            Method::Selector if !defs.iter().any(|d| d.name == c.value) => {
                return Some(at(
                    c,
                    format!("selectors.yml has no selector `{}`", c.value),
                ));
            }
            Method::ResourceType if ResourceType::parse(&c.value).is_none() => {
                return Some(at(
                    c,
                    format!(
                        "`{}` is no resource type (one of: {})",
                        c.value,
                        ResourceType::ALL.map(ResourceType::as_str).join(", ")
                    ),
                ));
            }
            _ => {}
        }
    }
    None
}

fn selected(n: &Node, root_package: &str) -> SelectedNode {
    let id_tail = match (&n.source, &n.version_number) {
        (Some((s, t)), _) => format!("{s}.{t}"),
        (None, Some(v)) => format!("{}.v{v}", n.name),
        (None, None) => n.name.clone(),
    };
    SelectedNode {
        uri: (!n.uri.as_os_str().is_empty()).then(|| cogitator_lsp::fs_path_to_uri(&n.uri)),
        name: n.name.clone(),
        label: n.display_name(),
        kind: n.resource_type.as_str(),
        resource_type: n.resource_type.as_str(),
        unique_id: format!("{}.{}.{id_tail}", n.resource_type.as_str(), n.package),
        version: n.version_number.clone(),
        package: n.package.clone(),
        path: n.path.clone(),
        tags: n.tags.clone(),
        range: n.range,
        graph_id: graph_id(n, root_package),
    }
}

/// The node's id in dbt.projectGraph: the root project's models (by file
/// stem), sources (by source), singular tests, analyses, exposures,
/// metrics and semantic models.
fn graph_id(n: &Node, root_package: &str) -> Option<String> {
    if n.package != root_package {
        return None;
    }
    let stem = || {
        n.uri
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .filter(|s| !s.is_empty())
    };
    match n.resource_type {
        ResourceType::Model => stem().map(|s| format!("model:{s}")),
        ResourceType::Source => n.source.as_ref().map(|(s, _)| format!("source:{s}")),
        ResourceType::Test if n.test.as_ref().is_some_and(|t| t.test_type == "singular") => {
            stem().map(|s| format!("test:{s}"))
        }
        ResourceType::Analysis => stem().map(|s| format!("analysis:{s}")),
        ResourceType::Exposure | ResourceType::Metric | ResourceType::SemanticModel => {
            Some(format!("{}:{}", n.resource_type.as_str(), n.name))
        }
        _ => None,
    }
}

/// The methods a person writes, and the values each takes in `u`.
pub fn vocabulary(u: &Universe, defs: &[Definition]) -> Vocabulary {
    let mut values: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    let mut add = |method: &'static str, value: &str| {
        if !value.is_empty() {
            values.entry(method).or_default().insert(value.to_string());
        }
    };
    for n in &u.nodes {
        for tag in &n.tags {
            add("tag", tag);
        }
        add("package", &n.package);
        add("resource_type", n.resource_type.as_str());
        if let Some(m) = n.config.get("materialized").and_then(|m| m.as_str()) {
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
            ResourceType::Source => {
                if let Some((source, table)) = &n.source {
                    add("source", source);
                    add("source", &format!("{source}.{table}"));
                }
            }
            ResourceType::Exposure => add("exposure", &n.name),
            ResourceType::Metric => add("metric", &n.name),
            ResourceType::SemanticModel => add("semantic_model", &n.name),
            ResourceType::UnitTest => add("unit_test", &n.name),
            ResourceType::Model | ResourceType::Seed | ResourceType::Snapshot => {
                add("fqn", &n.name)
            }
            _ => {}
        }
        if let Some(name) = n.test.as_ref().and_then(|t| t.test_name.as_deref()) {
            add("test_name", name.rsplit('.').next().unwrap_or(name));
        }
    }
    for d in defs {
        add("selector", &d.name);
    }
    for t in ["generic", "singular", "unit"] {
        add("test_type", t);
    }
    for v in ["latest", "prerelease", "old", "none"] {
        add("version", v);
    }
    add("package", "this");
    let mut methods: Vec<String> = METHODS
        .iter()
        .map(|(m, _)| *m)
        .filter(|m| {
            !matches!(
                *m,
                "config" | "state" | "result" | "source_status" | "wildcard"
            )
        })
        .map(str::to_string)
        .chain(["config.materialized".to_string(), "config.tags".to_string()])
        .collect();
    methods.sort_unstable();
    Vocabulary {
        methods,
        values: values
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.into_iter().collect()))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn universe() -> Universe {
        let mut u = Universe {
            root_package: "p".into(),
            ..Default::default()
        };
        let mut m = Node::new(ResourceType::Model, "orders", "p");
        m.uri = std::path::PathBuf::from("/p/models/orders.sql");
        m.tags = vec!["nightly".into()];
        u.push(m);
        u
    }

    fn ask(u: &Universe, select: &str, exclude: Option<&str>) -> SelectAnswer {
        select_nodes(
            u,
            &[],
            &SelectRequest {
                select,
                exclude,
                vocabulary: true,
            },
        )
    }

    #[test]
    fn answers_list_nodes_counts_and_graph_ids() {
        let u = universe();
        let answer = select_command(&u, &[], "orders").unwrap();
        assert_eq!(answer.count, 1);
        assert_eq!(answer.counts.get("model"), Some(&1));
        let orders = &answer.nodes[0];
        assert_eq!(orders.unique_id, "model.p.orders");
        assert_eq!(orders.graph_id.as_deref(), Some("model:orders"));
        assert!(orders
            .uri
            .as_deref()
            .unwrap()
            .ends_with("/p/models/orders.sql"));
        assert!(answer.vocabulary.is_none());
    }

    #[rstest::rstest]
    #[case("orders state:modified", None, 7, 21, "previous dbt run")]
    #[case("tag:nightly bogus:x", None, 12, 18, "no selection method")]
    #[case("2+bogus:x", None, 2, 8, "no selection method")]
    #[case("@+x", None, 0, 3, "cannot be combined")]
    #[case("resource_type:modle", None, 0, 19, "no resource type")]
    #[case("selector:nope", None, 0, 13, "no selector")]
    #[case("orders --exclude tag:", None, 17, 21, "no value")]
    #[case("orders --exclude a --exclude b", None, 19, 28, "second `--exclude`")]
    #[case("orders", Some("bogus:x"), 0, 6, "no selection method")]
    fn errors_say_what_and_where(
        #[case] text: &str,
        #[case] exclude: Option<&str>,
        #[case] start: usize,
        #[case] end: usize,
        #[case] says: &str,
    ) {
        let u = universe();
        let answer = ask(&u, text, exclude);
        let error = answer.error.clone().expect("an error");
        assert!(error.message.contains(says), "{}", error.message);
        assert_eq!((error.start, error.end), (start, end), "{}", error.message);
        assert!(answer.nodes.is_empty());
        let refused = [
            "previous dbt run",
            "no selection method",
            "no resource type",
            "no selector",
        ];
        if refused.contains(&says) {
            assert_eq!(
                answer.issues,
                vec![error.message],
                "a refusal is an issue too"
            );
        }
    }

    #[test]
    fn an_exclusion_subtracts_and_the_vocabulary_offers_tags() {
        let u = universe();
        assert!(ask(&u, "orders", Some("tag:nightly")).nodes.is_empty());
        assert!(ask(&u, "orders --exclude tag:nightly", None)
            .nodes
            .is_empty());
        assert_eq!(ask(&u, "orders --exclude tag:other", None).count, 1);
        let vocab = ask(&u, "orders", None).vocabulary.unwrap();
        assert_eq!(vocab.values["tag"], vec!["nightly"]);
        assert!(vocab.methods.contains(&"config.materialized".to_string()));
        assert!(!vocab.methods.contains(&"state".to_string()));
    }
}
