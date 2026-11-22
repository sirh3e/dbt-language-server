//! The operators that replace rows by group rather than add them:
//! aggregates (linear per group for SUM and COUNT, recomputed per touched
//! group otherwise) and window functions partitioned by a key.

use std::collections::BTreeMap;

use cogitator_ir::mir::{AggCall, ColId, Mir, ProvId, RelId, ScalarId, ScalarKind, WinCall};
use cogitator_ir::types::SqlType;

use super::scalar::{holistic, monotone_of};
use super::walk::{Group, Growth, Node, Walk};
use super::{Blocker, Note, Strategy};

impl Walk<'_> {
    pub(super) fn aggregate(
        &mut self,
        input: RelId,
        keys: &[(ColId, ScalarId)],
        aggs: &[(ColId, AggCall)],
        grouping_sets: bool,
        prov: ProvId,
    ) -> Node {
        let mir = self.mir;
        if grouping_sets {
            return self.block(Blocker::GroupingSets, prov);
        }
        let input = self.node(input);
        match (input.growth, &input.group) {
            (Growth::Static, _) => return Node::fixed(),
            (_, Some(_)) | (Growth::Changing, None) => {
                return self.block(Blocker::OverChangingRows("aggregate"), prov);
            }
            (Growth::Append, None) => {}
        }
        if keys.is_empty() {
            return self.block(Blocker::AggregateOverWholeTable, prov);
        }
        let mut mono = BTreeMap::new();
        let mut bucket = None;
        for (col, scalar) in keys {
            if let Some(source) = monotone_of(mir, *scalar, &input.mono) {
                mono.insert(*col, input.mono[&source]);
                bucket.get_or_insert(*col);
            }
        }
        let mut high_water = None;
        for (col, call) in aggs {
            let func = mir.syms.text(call.func).to_ascii_lowercase();
            let arg = call.args.first().copied();
            match func.as_str() {
                "max" | "min" => {
                    self.notes.insert(Note::MinMax);
                    let grows = arg.and_then(|a| monotone_of(mir, a, &input.mono));
                    if let (Some(source), "max") = (grows, func.as_str()) {
                        mono.insert(*col, input.mono[&source]);
                        high_water.get_or_insert(*col);
                    }
                }
                "sum" | "avg" if arg.is_some_and(|a| float(mir, a)) => {
                    self.notes.insert(Note::FloatSum);
                }
                _ if holistic(&func, call.distinct) => {
                    self.notes.insert(Note::Holistic(func.clone()));
                }
                _ => {}
            }
        }
        Node {
            growth: Growth::Changing,
            mono,
            group: Some(Group {
                keys: keys.iter().map(|(c, _)| *c).collect(),
                bucket,
                high_water,
                strategy: Strategy::DeleteInsert,
            }),
        }
    }

    pub(super) fn window(
        &mut self,
        input: RelId,
        funcs: &[(ColId, WinCall)],
        prov: ProvId,
    ) -> Node {
        let mir = self.mir;
        let node = self.node(input);
        match (node.growth, &node.group) {
            (Growth::Static, _) => return Node::fixed(),
            (_, Some(_)) | (Growth::Changing, None) => {
                return self.block(Blocker::OverChangingRows("window function"), prov);
            }
            (Growth::Append, None) => {}
        }
        // the partition keys every function shares
        let mut keys: Option<Vec<ColId>> = None;
        for (_, call) in funcs {
            let cols: Vec<ColId> = call
                .partition_by
                .iter()
                .filter_map(|s| match mir.scalars.get(*s).map(|s| &s.kind) {
                    Some(ScalarKind::Col(c)) => Some(*c),
                    _ => None,
                })
                .collect();
            keys = Some(match keys {
                None => cols,
                Some(prev) => prev.into_iter().filter(|c| cols.contains(c)).collect(),
            });
        }
        let keys = keys.unwrap_or_default();
        if keys.is_empty() {
            return self.block(Blocker::WindowOverWholeTable, prov);
        }
        let bucket = keys.iter().copied().find(|k| node.mono.contains_key(k));
        Node {
            growth: Growth::Changing,
            mono: node.mono,
            group: Some(Group {
                keys,
                bucket,
                high_water: None,
                strategy: Strategy::DeleteInsert,
            }),
        }
    }
}

fn float(mir: &Mir, scalar: ScalarId) -> bool {
    matches!(
        mir.scalars.get(scalar).map(|s| &s.ty.ty),
        Some(SqlType::Float { .. })
    )
}
