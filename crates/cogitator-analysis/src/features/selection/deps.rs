//! What a node depends on, read without rendering: the refs, sources and
//! macro calls of SQL, and what each macro of every project refs.

use std::collections::HashMap;

use cogitator_dialects::Dialect;
use cogitator_parser::{DocumentParser, JinjaSqlParser, TokenKind};

use super::projects::Project;
use super::universe::Dep;
use crate::features::quality::test_nodes::TestParent;
use crate::DbtContext;

pub(super) fn parent_dep(parent: &TestParent) -> Dep {
    match parent {
        TestParent::Node(m) => Dep::Model(m.clone()),
        TestParent::Source(s, t) => Dep::Source(s.clone(), t.clone()),
        TestParent::SourceTarget(target) => Dep::SourceTarget(target.clone()),
    }
}

/// The models and source tables a SQL text reads, and the macros it
/// calls (whose refs dbt counts as the caller's: it renders them).
pub(super) fn dependencies(text: &str, dialect: &Dialect) -> Vec<Dep> {
    let Ok(doc) = JinjaSqlParser.parse(text, dialect) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut source: Option<String> = None;
    for token in &doc.tokens {
        match token.kind {
            TokenKind::Ref => out.push(Dep::Model(doc.literal(token).to_string())),
            TokenKind::SourceName => source = Some(doc.literal(token).to_string()),
            TokenKind::SourceTable => {
                if let Some(src) = source.take() {
                    out.push(Dep::Source(src, doc.literal(token).to_string()));
                }
            }
            TokenKind::Macro => {
                let name = doc.literal(token);
                out.push(Dep::Macro(
                    name.rsplit('.').next().unwrap_or(name).to_string(),
                ));
            }
            _ => {}
        }
    }
    out
}

/// `{% macro name(…) %} … {% endmacro %}` blocks of a file: each name and
/// its body.
fn macro_blocks(text: &str) -> Vec<(String, &str)> {
    let mut out = Vec::new();
    let mut rest = text;
    let mut at = 0usize;
    while let Some(open) = rest.find("{%") {
        let after = rest[open + 2..].trim_start_matches('-').trim_start();
        if let Some(def) = after.strip_prefix("macro ") {
            let name: String = def
                .trim_start()
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            let body_start = at + open;
            let end = text[body_start..]
                .find("endmacro")
                .map_or(text.len(), |e| body_start + e);
            if !name.is_empty() {
                out.push((name, &text[body_start..end]));
            }
            at = end;
            rest = &text[end..];
            continue;
        }
        at += open + 2;
        rest = &text[at..];
    }
    out
}

/// Does the macro return before it reaches a ref when dbt parses it
/// (`{% if not execute %}{{ return(…) }}{% endif %}` ahead of its refs)?
/// dbt records the refs it renders while parsing, with `execute` false,
/// so such a macro's refs are none of its caller's.
fn returns_before_execute(body: &str) -> bool {
    let Some(guard) = body.find("if not execute") else {
        return false;
    };
    let first_ref = ["ref(", "source("]
        .iter()
        .filter_map(|call| body.find(call))
        .min()
        .unwrap_or(usize::MAX);
    let block = &body[guard..];
    let end = block.find("endif").unwrap_or(block.len());
    guard < first_ref && block[..end].contains("return(")
}

/// Every macro of every project: what it refs and which macros it calls.
pub(super) fn macros(ctx: &DbtContext, projects: &[Project]) -> HashMap<String, Vec<Dep>> {
    let mut out: HashMap<String, Vec<Dep>> = HashMap::new();
    for p in projects {
        for rel in &p.file.macro_paths {
            for file in crate::util::walk_filepath(&p.root.join(rel), ".sql") {
                let Ok(text) = std::fs::read_to_string(&file) else {
                    continue;
                };
                for (name, body) in macro_blocks(&text) {
                    let deps = if returns_before_execute(body) {
                        Vec::new()
                    } else {
                        dependencies(body, &ctx.dialect)
                    };
                    out.entry(name).or_default().extend(deps);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macro_blocks_split_a_file_by_macro() {
        let text = "{% macro a() %}{{ ref('x') }}{% endmacro %}\n{%- macro b(y) -%}{{ source('s','t') }}{%- endmacro -%}";
        let names: Vec<String> = macro_blocks(text).into_iter().map(|(n, _)| n).collect();
        assert_eq!(names, vec!["a", "b"]);
    }

    #[test]
    fn a_macro_that_returns_while_dbt_parses_refs_nothing() {
        assert!(returns_before_execute(
            "{% macro m() %}{%- if not execute -%}{{ return([]) }}{%- endif -%} select {{ source('fx', 'files') }}"
        ));
        assert!(!returns_before_execute(
            "{% macro m() %} select {{ ref('a') }} {% if not execute %}{{ return(1) }}{% endif %}"
        ));
        assert!(!returns_before_execute(
            "{% macro m() %} select {{ ref('a') }}"
        ));
    }
}
