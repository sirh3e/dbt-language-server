//! dbt's indirect selection of tests (`--indirect-selection`): which
//! tests come along with a selection, per criterion
//! (`expand_selection`) and at each set operation
//! (`incorporate_indirect_nodes`), and the graph walk both use.

use std::collections::BTreeSet;

use super::eval::Selected;
use super::syntax::Depth;
use super::universe::{ResourceType, Universe};

/// How tests come along with the nodes they test (dbt's
/// `--indirect-selection`, eager by default).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Indirect {
    Eager,
    Cautious,
    Buildable,
    Empty,
}

impl Indirect {
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "eager" => Indirect::Eager,
            "cautious" => Indirect::Cautious,
            "buildable" => Indirect::Buildable,
            "empty" => Indirect::Empty,
            _ => return None,
        })
    }
}

/// dbt's `expand_selection`: the tests downstream of `selected`.
/// Eager takes them all; cautious those whose parents are all
/// selected, buildable also those whose other parents are ancestors
/// of the selection; the rest wait as indirect.
pub(super) fn expand(u: &Universe, selected: BTreeSet<usize>, mode: Indirect) -> Selected {
    let mut out = Selected {
        direct: selected.clone(),
        indirect: BTreeSet::new(),
    };
    if mode == Indirect::Empty {
        return out;
    }
    let ancestors: BTreeSet<usize> = match mode {
        Indirect::Buildable => walk(&u.parents, &selected, None),
        _ => BTreeSet::new(),
    };
    for test in downstream_tests(u, &selected) {
        let parents = &u.parents[test];
        let covered = parents.iter().all(|p| selected.contains(p));
        let buildable = parents
            .iter()
            .all(|p| selected.contains(p) || ancestors.contains(p));
        if mode == Indirect::Eager || covered || (mode == Indirect::Buildable && buildable) {
            out.direct.insert(test);
        } else {
            out.indirect.insert(test);
        }
    }
    out
}

/// dbt's `incorporate_indirect_nodes`.
pub(super) fn incorporate(
    u: &Universe,
    direct: BTreeSet<usize>,
    indirect: &BTreeSet<usize>,
    mode: Indirect,
) -> BTreeSet<usize> {
    let ancestors = match mode {
        Indirect::Buildable => walk(&u.parents, &direct, None),
        Indirect::Cautious => BTreeSet::new(),
        Indirect::Eager | Indirect::Empty => return direct,
    };
    let mut out = direct.clone();
    for &test in indirect.difference(&direct) {
        if u.parents[test]
            .iter()
            .all(|p| direct.contains(p) || ancestors.contains(p))
        {
            out.insert(test);
        }
    }
    out
}

/// Tests (and unit tests) that test something in `selected`.
fn downstream_tests(u: &Universe, selected: &BTreeSet<usize>) -> Vec<usize> {
    let mut out: Vec<usize> = selected
        .iter()
        .flat_map(|&n| u.children[n].iter().copied())
        .filter(|c| !selected.contains(c))
        .filter(|&c| {
            matches!(
                u.nodes[c].resource_type,
                ResourceType::Test | ResourceType::UnitTest
            )
        })
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// The nodes `depth` steps away along `edges` (all of them for `None`).
pub(super) fn walk(edges: &[Vec<usize>], from: &BTreeSet<usize>, depth: Depth) -> BTreeSet<usize> {
    let mut seen = BTreeSet::new();
    let mut frontier: Vec<usize> = from.iter().copied().collect();
    let mut step = 0;
    while !frontier.is_empty() && depth.is_none_or(|d| step < d) {
        let mut next = Vec::new();
        for n in frontier {
            for &m in &edges[n] {
                if seen.insert(m) {
                    next.push(m);
                }
            }
        }
        frontier = next;
        step += 1;
    }
    seen
}
