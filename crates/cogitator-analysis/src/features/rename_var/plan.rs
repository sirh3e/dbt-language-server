//! The pure core of a var rename: which keys move and which scopes'
//! calls follow, from every same-name key and each scope's winner.

use std::collections::BTreeMap;
use std::path::PathBuf;

use cogitator_lsp::Range;

/// A var scope: the root project (`None`) or an installed package.
pub(crate) type Scope = Option<String>;

/// One key that defines a var, and the scopes that can see it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Entry {
    pub path: PathBuf,
    pub range: Range,
    pub visible: Vec<Scope>,
}

/// What a rename touches: entry indexes and the scopes whose calls move.
#[derive(Debug, PartialEq)]
pub(crate) struct Plan {
    pub keys: Vec<usize>,
    pub scopes: Vec<Scope>,
}

/// The pure core: `winners[s]` is the entry scope `s` resolves to.
pub(crate) fn plan(entries: &[Entry], winners: &BTreeMap<Scope, usize>, start: Start) -> Plan {
    let target = match &start {
        Start::Definition(d) => Some(*d),
        Start::Key(k) => {
            let k = *k;
            if winners.values().any(|&w| w == k) {
                Some(k)
            } else {
                // a shadowed key: the definition all its viewers share
                let mut seen = entries[k].visible.iter().map(|s| winners.get(s));
                match seen.next() {
                    Some(Some(&d)) if seen.all(|w| w == Some(&d)) => Some(d),
                    _ => None,
                }
            }
        }
        Start::Undefined(scope) => {
            return Plan {
                keys: Vec::new(),
                scopes: vec![scope.clone()],
            }
        }
    };
    let Some(d) = target else {
        let keys = match start {
            Start::Key(k) => vec![k],
            _ => Vec::new(),
        };
        return Plan {
            keys,
            scopes: Vec::new(),
        };
    };
    let scopes: Vec<Scope> = winners
        .iter()
        .filter(|(_, &w)| w == d)
        .map(|(s, _)| s.clone())
        .collect();
    let keys = (0..entries.len())
        .filter(|&i| {
            i == d
                || entries[i]
                    .visible
                    .iter()
                    .all(|s| winners.get(s).is_none_or(|&w| w == d))
        })
        .collect();
    Plan { keys, scopes }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Start {
    /// From a call that resolves to entry `d`.
    Definition(usize),
    /// From a call to an undefined var, in this scope: calls only.
    Undefined(Scope),
    /// From the key at entry `k`.
    Key(usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(visible: &[Option<&str>]) -> Entry {
        Entry {
            path: PathBuf::new(),
            range: Range::default(),
            visible: visible.iter().map(|s| s.map(str::to_string)).collect(),
        }
    }

    fn s(name: Option<&str>) -> Scope {
        name.map(str::to_string)
    }

    #[test]
    fn the_rename_follows_the_definition() {
        let root = None;
        let utils = Some("utils");
        // 0 root global (all), 1 root vars.shop (root), 2 root vars.utils (utils),
        // 3 override (all)
        let all = [root, utils];
        let entries = vec![entry(&all), entry(&[root]), entry(&[utils]), entry(&all)];
        let winners_of = |pairs: &[(Option<&str>, usize)]| -> BTreeMap<Scope, usize> {
            pairs.iter().map(|(sc, i)| (s(*sc), *i)).collect()
        };
        type Case<'a> = (Vec<(Option<&'a str>, usize)>, Start, Vec<usize>, Vec<Scope>);
        let cases: Vec<Case> = vec![
            // root scoped wins in root, global in utils: the scoped key alone
            (
                vec![(root, 1), (utils, 0)],
                Start::Definition(1),
                vec![1],
                vec![s(root)],
            ),
            // the global decides utils only (root shadows it with its
            // scoped key, which stays); a key only utils sees moves too
            (
                vec![(root, 1), (utils, 0)],
                Start::Definition(0),
                vec![0, 2],
                vec![s(utils)],
            ),
            // an override wins everywhere: every same-name key moves
            (
                vec![(root, 3), (utils, 3)],
                Start::Definition(3),
                vec![0, 1, 2, 3],
                vec![s(root), s(utils)],
            ),
            // a shadowed key: renames its viewers' shared definition
            (
                vec![(root, 3), (utils, 3)],
                Start::Key(1),
                vec![0, 1, 2, 3],
                vec![s(root), s(utils)],
            ),
            // viewers disagree: the key alone
            (vec![(root, 1), (utils, 2)], Start::Key(0), vec![0], vec![]),
            // an undefined var: the calls of that scope only
            (vec![], Start::Undefined(s(utils)), vec![], vec![s(utils)]),
        ];
        for (w, start, keys, scopes) in cases {
            let got = plan(&entries, &winners_of(&w), start.clone());
            assert_eq!(got, Plan { keys, scopes }, "{start:?} with {w:?}");
        }
    }
}
