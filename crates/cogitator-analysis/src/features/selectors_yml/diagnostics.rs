//! selectors.yml diagnostics: how a definition is written (unknown keys
//! and methods, a string that does not parse, duplicate names, more than
//! one default), what it references (a selector selectors.yml lacks, a
//! cycle, a method that needs state) and, given the project's nodes,
//! a definition that matches nothing.

use cogitator_lsp::{diagnostic_severity, Diagnostic, Range};

use crate::selectors::{self, Issue, Universe};

/// Diagnostics for how a selectors.yml text is written and what it
/// references: no project nodes needed (the push path).
pub fn selectors_diagnostics(text: &str) -> Vec<Diagnostic> {
    diagnostics(text, None)
}

/// The definitions of a selectors.yml text that match no node of `u`
/// (the pull path, which builds the universe).
pub fn unmatched_selector_diagnostics(text: &str, u: &Universe) -> Vec<Diagnostic> {
    diagnostics(text, Some(u))
        .into_iter()
        .filter(|d| d.severity == diagnostic_severity::WARNING)
        .collect()
}

/// Without a universe the "matches nothing" check is skipped.
fn diagnostics(text: &str, universe: Option<&Universe>) -> Vec<Diagnostic> {
    let file = selectors::yaml::parse(text);
    let mut out: Vec<Diagnostic> = file
        .problems
        .iter()
        .map(|p| diag(p.range, p.message.clone(), diagnostic_severity::ERROR))
        .collect();
    let empty = Universe::default();
    let u = universe.unwrap_or(&empty);
    for def in &file.definitions {
        let eval = selectors::select_definition(u, &file.definitions, &def.name);
        for issue in &eval.issues {
            let (range, severity) = match issue {
                Issue::NeedsState { range, .. } => (*range, diagnostic_severity::INFO),
                Issue::UnknownMethod { range, .. }
                | Issue::UnknownSelector { range, .. }
                | Issue::Cycle { range, .. } => (*range, diagnostic_severity::ERROR),
            };
            let d = diag(range, selectors::issue_message(issue), severity);
            if !out
                .iter()
                .any(|o| o.range == d.range && o.message == d.message)
            {
                out.push(d);
            }
        }
        let invalid = matches!(def.expr, selectors::Expr::Invalid);
        if universe.is_some() && eval.nodes.is_empty() && eval.issues.is_empty() && !invalid {
            out.push(diag(
                def.range,
                format!("selector `{}` matches no node of the project", def.name),
                diagnostic_severity::WARNING,
            ));
        }
    }
    out
}

fn diag(range: Range, message: String, severity: i32) -> Diagnostic {
    Diagnostic::plain(range, message, severity, "dbt-selectors", "cogitator")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selectors::{Node, ResourceType};

    const TEXT: &str = "\
selectors:
  - name: good
    definition: 'tag:x'
  - name: empty
    definition: 'tag:nothing'
  - name: odd
    definition:
      method: tags
      value: x
  - name: ref_lost
    definition: 'selector:nowhere'
  - name: a
    definition: 'selector:b'
  - name: b
    definition: 'selector:a'
  - name: old
    definition: 'state:modified'
";

    fn universe() -> Universe {
        let mut u = Universe::default();
        let mut n = Node::new(ResourceType::Model, "m", "p");
        n.tags = vec!["x".into()];
        u.push(n);
        u
    }

    #[test]
    fn every_kind_of_problem() {
        let u = universe();
        let got: Vec<(u32, i32, String)> = diagnostics(TEXT, Some(&u))
            .into_iter()
            .map(|d| (d.range.start.line, d.severity, d.message))
            .collect();
        let lines: Vec<(u32, i32, &str)> = got
            .iter()
            .map(|(l, s, m)| (*l, *s, m.split(" (known").next().unwrap()))
            .collect();
        assert_eq!(
            lines,
            vec![
                (3, 2, "selector `empty` matches no node of the project"),
                (7, 1, "unknown selector method `tags`"),
                (10, 1, "selectors.yml defines no selector `nowhere`"),
                (14, 1, "selector cycle: a → b → a"),
                (12, 1, "selector cycle: b → a → b"),
                (16, 3, "`state:` needs a previous run's artifacts (dbt --state); it selects nothing here"),
            ]
        );
    }

    #[test]
    fn without_nodes_there_is_no_emptiness_warning() {
        let got = selectors_diagnostics(TEXT);
        assert!(got.iter().all(|d| !d.message.contains("matches no node")));
        assert!(got.iter().any(|d| d.message.contains("tags")));
        let unmatched = unmatched_selector_diagnostics(TEXT, &universe());
        assert_eq!(unmatched.len(), 1);
        assert!(unmatched[0].message.contains("`empty`"));
    }
}
