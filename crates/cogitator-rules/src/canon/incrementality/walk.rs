//! The bottom-up walk: each relation's increment from its inputs', by the
//! DBSP class of its operator (see `mod.rs`).

use std::collections::{BTreeMap, BTreeSet};

use cogitator_ir::common::SetOpKind;
use cogitator_ir::mir::{ColId, Mir, ProvId, RelId, RelOp};

use super::scalar::facts;
use super::{Analysis, Blocked, Blocker, DeltaInput, IncrementalInputs, Note, Strategy, Verdict};

/// How a relation's rows change from one run to the next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Growth {
    /// Never changes (a dimension, or assumed to be one).
    Static,
    /// Only gains rows: its increment is its new rows.
    Append,
    /// Rows change (groups are re-aggregated): its increment replaces rows.
    Changing,
}

/// A relation replaced by group: the output columns that identify a group.
#[derive(Debug, Clone)]
pub(super) struct Group {
    pub(super) keys: Vec<ColId>,
    /// A key that is a bucket of the event column.
    pub(super) bucket: Option<ColId>,
    /// An output column holding each group's newest event time.
    pub(super) high_water: Option<ColId>,
    pub(super) strategy: Strategy,
}

#[derive(Debug, Clone)]
pub(super) struct Node {
    pub(super) growth: Growth,
    /// Output columns that only grow, to the driving input they track.
    pub(super) mono: BTreeMap<ColId, usize>,
    pub(super) group: Option<Group>,
}

impl Node {
    pub(super) fn fixed() -> Self {
        Node {
            growth: Growth::Static,
            mono: BTreeMap::new(),
            group: None,
        }
    }
}

pub(super) struct Walk<'a> {
    pub(super) mir: &'a Mir,
    pub(super) inputs: Option<&'a IncrementalInputs>,
    pub(super) memo: BTreeMap<RelId, Node>,
    pub(super) drivers: Vec<DeltaInput>,
    pub(super) blockers: Vec<Blocked>,
    pub(super) notes: BTreeSet<Note>,
    pub(super) unknown: bool,
}

pub fn classify(mir: &Mir, inputs: Option<&IncrementalInputs>) -> Analysis {
    let mut walk = Walk {
        mir,
        inputs,
        memo: BTreeMap::new(),
        drivers: Vec::new(),
        blockers: Vec::new(),
        notes: BTreeSet::new(),
        unknown: false,
    };
    let root = walk.node(mir.root);
    let verdict = walk.verdict(&root);
    Analysis {
        verdict,
        notes: walk.notes.into_iter().collect(),
    }
}

impl Walk<'_> {
    pub(super) fn node(&mut self, id: RelId) -> Node {
        if let Some(node) = self.memo.get(&id) {
            return node.clone();
        }
        let node = self.compute(id);
        self.memo.insert(id, node.clone());
        node
    }

    pub(super) fn block(&mut self, blocker: Blocker, prov: ProvId) -> Node {
        let blocked = Blocked { blocker, prov };
        if !self.blockers.contains(&blocked) {
            self.blockers.push(blocked);
        }
        Node::fixed()
    }

    pub(super) fn unknown(&mut self) -> Node {
        self.unknown = true;
        Node::fixed()
    }

    fn compute(&mut self, id: RelId) -> Node {
        let mir = self.mir;
        let Some(rel) = mir.rels.get(id) else {
            return self.unknown();
        };
        let prov = rel.prov;
        match &rel.op {
            RelOp::Scan { table, cols, open } => self.scan(*table, cols, *open, prov),
            RelOp::Filter { input, conjuncts } => {
                let node = self.node(*input);
                for scalar in conjuncts {
                    let facts = facts(mir, *scalar);
                    if let Some(name) = facts.nondeterministic {
                        return self.block(Blocker::TemporalFilter(name), prov);
                    }
                    if !self.subqueries_static(&facts.subqueries, facts.opaque, prov) {
                        return Node::fixed();
                    }
                }
                if node.group.is_some() {
                    // HAVING: a group may drop out of the output
                    self.notes.insert(Note::Having);
                    let mut node = node;
                    if let Some(group) = node.group.as_mut() {
                        group.strategy = Strategy::DeleteInsert;
                    }
                    return node;
                }
                node
            }
            RelOp::Project { input, items } => self.project(*input, items, prov),
            RelOp::Join { kind, l, r, on } => {
                for scalar in on {
                    let facts = facts(mir, *scalar);
                    if !self.subqueries_static(&facts.subqueries, facts.opaque, prov) {
                        return Node::fixed();
                    }
                }
                self.join(*kind, *l, *r, prov)
            }
            RelOp::Aggregate {
                input,
                keys,
                aggs,
                sets,
            } => self.aggregate(*input, keys, aggs, sets.is_some(), prov),
            RelOp::Window { input, funcs } => self.window(*input, funcs, prov),
            RelOp::Distinct(input) => self.distinct(*input, &rel.out, prov),
            RelOp::SetOp {
                op,
                all,
                inputs,
                out,
            } => match (op, all) {
                (SetOpKind::Union, true) => self.union_all(inputs, out, prov),
                (SetOpKind::Union, false) => {
                    let node = self.union_all(inputs, out, prov);
                    if node.growth == Growth::Static {
                        return node;
                    }
                    self.distinct_of(node, out)
                }
                _ => self.block(Blocker::SetDifference, prov),
            },
            RelOp::Sort { input, .. } => self.node(*input),
            RelOp::Limit { input, .. } => {
                let node = self.node(*input);
                if node.growth == Growth::Static {
                    return node;
                }
                self.block(Blocker::Limit, prov)
            }
            RelOp::Unnest { input, .. } => {
                // row by row: linear
                self.node(*input)
            }
            RelOp::CteRef { cte, out } => {
                let Some(def) = mir.ctes.get(*cte) else {
                    return self.unknown();
                };
                let body_out = mir
                    .rels
                    .get(def.body)
                    .map(|r| r.out.clone())
                    .unwrap_or_default();
                let node = self.node(def.body);
                let map: BTreeMap<ColId, ColId> =
                    body_out.iter().copied().zip(out.iter().copied()).collect();
                remap(node, &map)
            }
            RelOp::Opaque { .. } => self.unknown(),
        }
    }

    fn verdict(&mut self, root: &Node) -> Verdict {
        if !self.blockers.is_empty() {
            return Verdict::Keep(std::mem::take(&mut self.blockers));
        }
        if self.unknown {
            return Verdict::Unknown;
        }
        let mir = self.mir;
        let out = mir.root_columns();
        let name = |c: ColId| -> Option<String> {
            out.contains(&c)
                .then(|| mir.cols.get(c).and_then(|d| d.name))
                .flatten()
                .map(|s| mir.syms.text(s).to_string())
        };
        let root_prov = mir.rels.get(mir.root).map(|r| r.prov);
        let keep = |blocker| {
            Verdict::Keep(
                root_prov
                    .map(|prov| Blocked { blocker, prov })
                    .into_iter()
                    .collect(),
            )
        };
        match (root.growth, &root.group) {
            (Growth::Static, _) => Verdict::Static,
            (_, Some(group)) => {
                let keys: Option<Vec<String>> = group.keys.iter().map(|k| name(*k)).collect();
                let Some(keys) = keys else {
                    return keep(Blocker::KeysNotInOutput);
                };
                let driver = group
                    .bucket
                    .and_then(|b| root.mono.get(&b))
                    .or_else(|| root.mono.values().next())
                    .and_then(|d| self.drivers.get(*d))
                    .cloned();
                let Some(input) = driver.or_else(|| self.drivers.first().cloned()) else {
                    return keep(Blocker::NoMonotoneColumn);
                };
                match group.bucket.and_then(name) {
                    Some(bucket) => Verdict::ByBucket {
                        input,
                        bucket,
                        keys,
                    },
                    None => Verdict::ByKey {
                        input,
                        keys,
                        strategy: group.strategy,
                        high_water: group.high_water.and_then(name),
                    },
                }
            }
            (Growth::Append, None) => {
                // the out column that tracks a driving input, declared first
                let mut found: Vec<(bool, String)> = root
                    .mono
                    .iter()
                    .filter_map(|(c, d)| Some((self.drivers.get(*d)?.declared, name(*c)?)))
                    .collect();
                found.sort_by_key(|f| std::cmp::Reverse(f.0));
                match found.into_iter().next() {
                    Some((_, out_column)) => Verdict::Append {
                        inputs: self.drivers.clone(),
                        out_column,
                    },
                    None => keep(Blocker::NoMonotoneColumn),
                }
            }
            (Growth::Changing, None) => keep(Blocker::KeysNotInOutput),
        }
    }
}

fn remap(node: Node, map: &BTreeMap<ColId, ColId>) -> Node {
    let get = |c: ColId| map.get(&c).copied();
    Node {
        growth: node.growth,
        mono: node
            .mono
            .into_iter()
            .filter_map(|(c, d)| Some((get(c)?, d)))
            .collect(),
        group: node.group.and_then(|g| {
            Some(Group {
                keys: g.keys.iter().map(|k| get(*k)).collect::<Option<_>>()?,
                bucket: g.bucket.and_then(get),
                high_water: g.high_water.and_then(get),
                strategy: g.strategy,
            })
        }),
    }
}
