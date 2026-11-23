//! Evaluating a selection over a [`Universe`], as dbt-core's
//! `NodeSelector` does: each criterion's nodes plus its graph operators,
//! unions, intersections and differences of them, `selector:`
//! references to other definitions, and indirect selection of tests.

use std::collections::BTreeSet;

use cogitator_lsp::Range;

use super::indirect;
pub use super::indirect::Indirect;
use super::matcher;
use super::method::Method;
use super::syntax::{Criterion, Selection};
use super::universe::Universe;
use super::yaml::{Definition, Expr};

/// What evaluation could not do, at the place it is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Issue {
    UnknownMethod {
        name: String,
        range: Range,
    },
    UnknownSelector {
        name: String,
        range: Range,
    },
    /// A definition that reaches itself: the chain of names.
    Cycle {
        chain: Vec<String>,
        range: Range,
    },
    /// `state:`, `result:`, `source_status:`: evaluated as nothing.
    NeedsState {
        method: String,
        range: Range,
    },
}

#[derive(Debug, Clone, Default)]
pub struct Evaluation {
    pub nodes: BTreeSet<usize>,
    pub issues: Vec<Issue>,
}

/// dbt's (direct, indirect) pair: indirect are tests that join only if
/// the final selection covers what they test (cautious, buildable).
#[derive(Default, Clone)]
pub(super) struct Selected {
    pub(super) direct: BTreeSet<usize>,
    pub(super) indirect: BTreeSet<usize>,
}

/// Evaluate a `--select` string. `defs` resolves `selector:` references.
pub fn select(
    u: &Universe,
    defs: &[Definition],
    selection: &Selection,
    indirect: Indirect,
) -> Evaluation {
    let expr = expr_of(selection);
    run(u, defs, indirect, |ev| ev.expr(&expr))
}

/// Evaluate a selectors.yml definition by name.
pub fn select_definition(u: &Universe, defs: &[Definition], name: &str) -> Evaluation {
    run(u, defs, Indirect::Eager, |ev| {
        ev.reference(name, Range::default())
    })
}

/// The expression of a parsed string: a union of intersections.
pub fn expr_of(selection: &Selection) -> Expr {
    Expr::Union(
        selection
            .union
            .iter()
            .map(|i| {
                Expr::Intersection(
                    i.iter()
                        .map(|c| Expr::Criterion {
                            criterion: c.clone(),
                            indirect: None,
                            method_range: Range::default(),
                            value_range: Range::default(),
                        })
                        .collect(),
                )
            })
            .collect(),
    )
}

fn run(
    u: &Universe,
    defs: &[Definition],
    indirect: Indirect,
    f: impl FnOnce(&mut Evaluator<'_>) -> Selected,
) -> Evaluation {
    let mut ev = Evaluator {
        u,
        defs,
        indirect,
        stack: Vec::new(),
        issues: Vec::new(),
    };
    let nodes = f(&mut ev).direct;
    Evaluation {
        nodes,
        issues: ev.issues,
    }
}

struct Evaluator<'a> {
    u: &'a Universe,
    defs: &'a [Definition],
    indirect: Indirect,
    stack: Vec<String>,
    issues: Vec<Issue>,
}

impl Evaluator<'_> {
    fn issue(&mut self, issue: Issue) {
        if !self.issues.contains(&issue) {
            self.issues.push(issue);
        }
    }

    fn expr(&mut self, expr: &Expr) -> Selected {
        match expr {
            Expr::Criterion {
                criterion,
                indirect,
                method_range,
                value_range,
            } => self.criterion(
                criterion,
                indirect.unwrap_or(self.indirect),
                *method_range,
                *value_range,
            ),
            Expr::Union(parts) => {
                let parts: Vec<Selected> = parts.iter().map(|p| self.expr(p)).collect();
                self.combine(parts, |sets| sets.into_iter().flatten().collect())
            }
            Expr::Intersection(parts) => {
                let parts: Vec<Selected> = parts.iter().map(|p| self.expr(p)).collect();
                self.combine(parts, |sets| {
                    let mut iter = sets.into_iter();
                    let first = iter.next().unwrap_or_default();
                    iter.fold(first, |acc, s| acc.intersection(&s).copied().collect())
                })
            }
            Expr::Difference(base, excluded) => {
                let parts = vec![self.expr(base), self.expr(excluded)];
                self.combine(parts, |sets| {
                    let mut iter = sets.into_iter();
                    let first = iter.next().unwrap_or_default();
                    let gone: BTreeSet<usize> = iter.flatten().collect();
                    first.difference(&gone).copied().collect()
                })
            }
            Expr::Invalid => Selected::default(),
        }
    }

    fn criterion(
        &mut self,
        c: &Criterion,
        indirect: Indirect,
        method_range: Range,
        value_range: Range,
    ) -> Selected {
        let base: BTreeSet<usize> = match &c.method {
            Method::Selector => return self.reference(&c.value, value_range),
            Method::NeedsState(name) => {
                self.issue(Issue::NeedsState {
                    method: (*name).to_string(),
                    range: method_range,
                });
                return Selected::default();
            }
            Method::Unknown(name) => {
                self.issue(Issue::UnknownMethod {
                    name: name.clone(),
                    range: method_range,
                });
                return Selected::default();
            }
            method => matcher::search(self.u, method, &c.value)
                .unwrap_or_default()
                .into_iter()
                .collect(),
        };
        let mut direct = base.clone();
        if c.childrens_parents {
            let down = indirect::walk(&self.u.children, &base, None);
            let around: BTreeSet<usize> = base.union(&down).copied().collect();
            direct.extend(&down);
            direct.extend(indirect::walk(&self.u.parents, &around, None));
        }
        if let Some(depth) = c.parents {
            direct.extend(indirect::walk(&self.u.parents, &base, depth));
        }
        if let Some(depth) = c.children {
            direct.extend(indirect::walk(&self.u.children, &base, depth));
        }
        indirect::expand(self.u, direct, indirect)
    }

    /// dbt's `SelectionUnion`/`Intersection`/`Difference`: combine the
    /// direct sets, and the direct-or-indirect sets, then let indirect
    /// tests in whose parents the combined selection covers.
    fn combine(
        &self,
        parts: Vec<Selected>,
        op: impl Fn(Vec<BTreeSet<usize>>) -> BTreeSet<usize>,
    ) -> Selected {
        let direct = op(parts.iter().map(|p| p.direct.clone()).collect());
        let wider = op(parts
            .iter()
            .map(|p| p.direct.union(&p.indirect).copied().collect())
            .collect());
        let direct = indirect::incorporate(self.u, direct, &wider, self.indirect);
        let indirect = wider.difference(&direct).copied().collect();
        Selected { direct, indirect }
    }

    /// A `selector:` reference: its definition's nodes, or an issue for
    /// a name selectors.yml lacks or a definition that reaches itself.
    fn reference(&mut self, name: &str, range: Range) -> Selected {
        if let Some(at) = self.stack.iter().position(|n| n == name) {
            let mut chain = self.stack[at..].to_vec();
            chain.push(name.to_string());
            self.issue(Issue::Cycle { chain, range });
            return Selected::default();
        }
        let Some(def) = self.defs.iter().find(|d| d.name == name) else {
            self.issue(Issue::UnknownSelector {
                name: name.to_string(),
                range,
            });
            return Selected::default();
        };
        self.stack.push(name.to_string());
        let out = self.expr(&def.expr);
        self.stack.pop();
        out
    }
}

#[cfg(test)]
#[path = "eval_tests.rs"]
mod tests;
