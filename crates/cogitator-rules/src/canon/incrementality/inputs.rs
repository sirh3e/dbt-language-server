//! The leaves and row-by-row operators of the walk: what a scan reads
//! and how it grows, projections, and subqueries in scalars.

use std::collections::BTreeMap;

use cogitator_ir::mir::{ColId, ProvId, RelId, ScalarId, ScalarKind, TableId, TableRef};

use super::scalar::{facts, looks_like_event_time, monotone_of, temporal};
use super::walk::{Group, Growth, Node, Walk};
use super::{Blocker, DeltaInput, InputGrowth, Note};

impl Walk<'_> {
    pub(super) fn scan(
        &mut self,
        table: TableId,
        cols: &[ColId],
        open: bool,
        prov: ProvId,
    ) -> Node {
        let mir = self.mir;
        let Some(def) = mir.tables.get(table) else {
            return self.unknown();
        };
        let relation = match &def.target {
            TableRef::Model { name, .. } => mir.syms.text(*name).to_string(),
            TableRef::Source { source, table } => {
                format!("{}.{}", mir.syms.text(*source), mir.syms.text(*table))
            }
            TableRef::This | TableRef::Jinja => return self.unknown(),
            TableRef::Physical(parts) => parts
                .iter()
                .map(|p| mir.syms.text(*p))
                .collect::<Vec<_>>()
                .join("."),
        };
        if open {
            return self.unknown();
        }
        let declared = self
            .inputs
            .and_then(|inputs| inputs.by_name.get(relation.as_str()))
            .cloned();
        if declared.as_ref().map(|f| f.growth) == Some(InputGrowth::Changing) {
            return self.block(Blocker::ChangingInput(relation), prov);
        }
        let name_of = |c: &ColId| {
            mir.cols
                .get(*c)
                .and_then(|d| d.name)
                .map(|s| mir.syms.text(s))
        };
        let declared_col = declared.as_ref().and_then(|f| f.event_time.clone());
        let event = match &declared_col {
            Some(want) => cols
                .iter()
                .find(|c| name_of(c).is_some_and(|n| n.eq_ignore_ascii_case(want)))
                .map(|c| (*c, true)),
            None => None,
        }
        .or_else(|| {
            cols.iter()
                .find(|c| {
                    let temporal_col = mir.cols.get(**c).is_some_and(|d| temporal(&d.ty.ty));
                    temporal_col && name_of(c).is_some_and(looks_like_event_time)
                })
                .map(|c| (*c, false))
        });
        let append_declared = declared.as_ref().map(|f| f.growth) == Some(InputGrowth::Append);
        let Some((col, by_declaration)) = event else {
            if append_declared {
                // grows, but by no column the analysis can read back
                return self.block(Blocker::NoMonotoneColumn, prov);
            }
            self.notes.insert(Note::AssumesStatic(relation));
            return Node::fixed();
        };
        let driver = self.drivers.len();
        self.drivers.push(DeltaInput {
            relation,
            column: name_of(&col).unwrap_or_default().to_string(),
            declared: by_declaration || append_declared,
        });
        Node {
            growth: Growth::Append,
            mono: BTreeMap::from([(col, driver)]),
            group: None,
        }
    }

    pub(super) fn project(
        &mut self,
        input: RelId,
        items: &[(ColId, ScalarId)],
        prov: ProvId,
    ) -> Node {
        let mir = self.mir;
        let node = self.node(input);
        let mut mono = BTreeMap::new();
        let mut map = BTreeMap::new();
        for (col, scalar) in items {
            let facts = facts(mir, *scalar);
            if node.growth != Growth::Static {
                if let Some(name) = facts.nondeterministic {
                    return self.block(Blocker::NonDeterministic(name), prov);
                }
            }
            if !self.subqueries_static(&facts.subqueries, facts.opaque, prov) {
                return Node::fixed();
            }
            if let Some(ScalarKind::Col(source)) = mir.scalars.get(*scalar).map(|s| &s.kind) {
                map.insert(*source, *col);
            }
            if let Some(source) = monotone_of(mir, *scalar, &node.mono) {
                mono.insert(*col, node.mono[&source]);
            }
        }
        let group = node.group.and_then(|g| {
            let keys: Option<Vec<ColId>> = g.keys.iter().map(|k| map.get(k).copied()).collect();
            Some(Group {
                keys: keys?,
                bucket: g.bucket.and_then(|b| map.get(&b).copied()),
                high_water: g.high_water.and_then(|h| map.get(&h).copied()),
                strategy: g.strategy,
            })
        });
        if node.growth == Growth::Changing && group.is_none() {
            return self.block(Blocker::KeysNotInOutput, prov);
        }
        Node {
            growth: node.growth,
            mono,
            group,
        }
    }
    /// Subqueries over inputs that only grow change which old rows pass:
    /// the increment would miss them. Unknown scalars give no verdict.
    pub(super) fn subqueries_static(
        &mut self,
        subqueries: &[RelId],
        opaque: bool,
        prov: ProvId,
    ) -> bool {
        if opaque {
            self.unknown = true;
            return false;
        }
        for rel in subqueries {
            if self.node(*rel).growth != Growth::Static {
                self.block(Blocker::SubqueryOverGrowingInput, prov);
                return false;
            }
        }
        true
    }
}
