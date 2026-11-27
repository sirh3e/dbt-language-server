//! A model's refs to itself and the refs that close a cycle: dbt refuses
//! the whole project with "Found a cycle" for either.

use std::collections::{HashMap, HashSet, VecDeque};

use cogitator_parser::TokenKind;

use super::{finding, DbtContext, Document, Finding, Fix, ERROR, REF_CYCLE_CODE, SELF_REF_CODE};
use crate::graph::NodeKind;

/// How many models a cycle search walks at most: beyond it, no finding.
const SEARCH_LIMIT: usize = 20_000;

pub(crate) fn check(ctx: &DbtContext, doc: &Document, model: &str) -> Vec<Finding> {
    let text: &str = &doc.text;
    let mut reported = HashSet::new();
    let mut out = Vec::new();
    for token in doc
        .parsed
        .tokens
        .iter()
        .filter(|t| t.kind == TokenKind::Ref)
    {
        let target = doc.parsed.literal(token);
        let span = (token.start as usize, token.end as usize);
        let Some(call) = ref_call(text, span) else {
            continue;
        };
        if call.package.is_some_and(|p| p != ctx.project.name) {
            continue;
        }
        if target == model {
            out.push(
                finding(
                    span,
                    SELF_REF_CODE,
                    ERROR,
                    format!("model '{model}' refs itself"),
                )
                .because("dbt refuses a model that depends on itself (a cycle)")
                .fixed_by(Fix::Rewrite {
                    title: "Use {{ this }} for the model's own relation".to_string(),
                    start: call.start,
                    end: call.end,
                    text: "this".to_string(),
                }),
            );
            continue;
        }
        if !reported.insert(target.to_string()) {
            continue;
        }
        if let Some(path) = cycle(ctx, model, target) {
            out.push(
                finding(
                    span,
                    REF_CYCLE_CODE,
                    ERROR,
                    format!("ref('{target}') closes a cycle: {}", path.join(" → ")),
                )
                .because("dbt refuses the project when models depend on each other in a circle")
                .solved_by(format!(
                    "drop this ref, or the ref back to '{model}' further down the cycle"
                )),
            );
        }
    }
    out
}

/// One `ref(...)` call around a model-name token.
struct RefCall<'a> {
    /// From `ref` to past its `)`.
    start: usize,
    end: usize,
    /// The package of a two-argument `ref('package', 'model')`.
    package: Option<&'a str>,
}

/// The `ref(...)` call whose model name is at `span`.
fn ref_call(text: &str, (start, end): (usize, usize)) -> Option<RefCall<'_>> {
    let open = text[..start].rfind('(')?;
    let name = text[..open].trim_end();
    let call_start = name.strip_suffix("ref").map(str::len)?;
    if text[..call_start]
        .chars()
        .next_back()
        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
    {
        return None;
    }
    let close = end + text[end..].find(')')?;
    let before = text[open + 1..start].trim();
    // the model's own quote is the last character before it
    let before = before
        .strip_suffix(['\'', '"'])
        .unwrap_or(before)
        .trim_end();
    let package = before
        .strip_suffix(',')
        .map(str::trim)
        .and_then(|p| p.strip_prefix(['\'', '"']))
        .and_then(|p| p.strip_suffix(['\'', '"']));
    Some(RefCall {
        start: call_start,
        end: close + 1,
        package,
    })
}

/// The models from `model` through `target` back to `model`, when
/// `target`'s upstream reaches `model` (the shortest such way).
fn cycle(ctx: &DbtContext, model: &str, target: &str) -> Option<Vec<String>> {
    ctx.graph.node(NodeKind::Model, target)?;
    let mut parent: HashMap<String, String> = HashMap::new();
    let mut queue = VecDeque::from([target.to_string()]);
    let mut seen = HashSet::from([target.to_string()]);
    while let Some(current) = queue.pop_front() {
        if seen.len() > SEARCH_LIMIT {
            return None;
        }
        for next in ctx
            .graph
            .upstream_of(&current)
            .filter(|n| n.kind == NodeKind::Model)
        {
            if next.name == model {
                let mut path = vec![model.to_string(), model.to_string()];
                let mut at = current.clone();
                let mut middle = vec![at.clone()];
                while let Some(p) = parent.get(&at) {
                    middle.push(p.clone());
                    at = p.clone();
                }
                middle.reverse();
                path.splice(1..1, middle);
                return Some(path);
            }
            if seen.insert(next.name.clone()) {
                parent.insert(next.name.clone(), current.clone());
                queue.push_back(next.name.clone());
            }
        }
    }
    None
}
