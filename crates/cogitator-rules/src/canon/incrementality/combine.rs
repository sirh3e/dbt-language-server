//! The operators that combine relations: joins (bilinear), DISTINCT and
//! UNION ALL.

use std::collections::BTreeMap;

use cogitator_ir::common::JoinKind;
use cogitator_ir::mir::{ColId, ProvId, RelId};

use super::walk::{Group, Growth, Node, Walk};
use super::{Blocker, Note, Strategy};

impl Walk<'_> {
    pub(super) fn join(&mut self, kind: JoinKind, l: RelId, r: RelId, prov: ProvId) -> Node {
        let (a, b) = (self.node(l), self.node(r));
        let mut mono = a.mono.clone();
        mono.extend(b.mono.clone());
        use Growth::*;
        let growth = match (a.growth, b.growth) {
            (Static, Static) => return Node::fixed(),
            // a re-aggregated side joined to a dimension keeps its groups
            (Changing, Static) if matches!(kind, JoinKind::Inner | JoinKind::Left) => {
                return Node {
                    growth: Changing,
                    mono,
                    group: a.group,
                };
            }
            (Static, Changing) if matches!(kind, JoinKind::Inner | JoinKind::Right) => {
                return Node {
                    growth: Changing,
                    mono,
                    group: b.group,
                };
            }
            (Changing, _) | (_, Changing) => {
                return self.block(Blocker::OverChangingRows("join"), prov);
            }
            (Append, Static) => match kind {
                JoinKind::Inner
                | JoinKind::Cross
                | JoinKind::Left
                | JoinKind::Semi
                | JoinKind::Anti => Append,
                JoinKind::Right => return self.block(Blocker::OuterJoinOnGrowingSide, prov),
                JoinKind::Full => return self.block(Blocker::FullJoin, prov),
            },
            (Static, Append) => match kind {
                JoinKind::Inner | JoinKind::Cross | JoinKind::Right => Append,
                JoinKind::Left => return self.block(Blocker::OuterJoinOnGrowingSide, prov),
                JoinKind::Semi | JoinKind::Anti => {
                    return self.block(Blocker::SubqueryOverGrowingInput, prov)
                }
                JoinKind::Full => return self.block(Blocker::FullJoin, prov),
            },
            (Append, Append) => match kind {
                JoinKind::Inner | JoinKind::Cross => {
                    if let Some(other) = self.drivers.get(*b.mono.values().next().unwrap_or(&0)) {
                        let other = other.relation.clone();
                        self.notes.insert(Note::Bilinear { other });
                    }
                    Append
                }
                JoinKind::Full => return self.block(Blocker::FullJoin, prov),
                _ => return self.block(Blocker::OuterJoinOnGrowingSide, prov),
            },
        };
        Node {
            growth,
            mono,
            group: None,
        }
    }

    pub(super) fn distinct(&mut self, input: RelId, out: &[ColId], prov: ProvId) -> Node {
        let node = self.node(input);
        match (node.growth, &node.group) {
            (Growth::Static, _) => node,
            (Growth::Append, None) => self.distinct_of(node, out),
            _ => self.block(Blocker::OverChangingRows("distinct"), prov),
        }
    }

    /// DISTINCT of an append-only input only gains rows; they are merged
    /// on every column so a row seen before is not added twice.
    pub(super) fn distinct_of(&mut self, node: Node, out: &[ColId]) -> Node {
        Node {
            growth: Growth::Changing,
            group: Some(Group {
                keys: out.to_vec(),
                bucket: out.iter().copied().find(|c| node.mono.contains_key(c)),
                high_water: None,
                strategy: Strategy::Merge,
            }),
            mono: node.mono,
        }
    }

    pub(super) fn union_all(&mut self, inputs: &[RelId], out: &[ColId], prov: ProvId) -> Node {
        let mir = self.mir;
        let nodes: Vec<(Node, Vec<ColId>)> = inputs
            .iter()
            .map(|id| {
                let cols = mir.rels.get(*id).map(|r| r.out.clone()).unwrap_or_default();
                (self.node(*id), cols)
            })
            .collect();
        if nodes.iter().all(|(n, _)| n.growth == Growth::Static) {
            return Node::fixed();
        }
        if nodes.iter().any(|(n, _)| n.growth == Growth::Changing) {
            return self.block(Blocker::OverChangingRows("union"), prov);
        }
        if nodes.iter().any(|(n, _)| n.growth == Growth::Static) {
            return self.block(Blocker::StaticUnionBranch, prov);
        }
        // a position grows when it grows in every branch
        let mut mono = BTreeMap::new();
        for (i, col) in out.iter().enumerate() {
            let drivers: Option<Vec<usize>> = nodes
                .iter()
                .map(|(n, cols)| cols.get(i).and_then(|c| n.mono.get(c)).copied())
                .collect();
            if let Some(first) = drivers.and_then(|d| d.first().copied()) {
                mono.insert(*col, first);
            }
        }
        Node {
            growth: Growth::Append,
            mono,
            group: None,
        }
    }
}
