//! What the pass manager hands each rule.

use std::collections::BTreeMap;
use std::sync::Arc;

use cogitator_ir::common::{Materialization, ModelConfig};
use cogitator_ir::hir::Hir;
use cogitator_ir::lir::{Backend, PhysicalFacts};
use cogitator_ir::mir::Mir;
use cogitator_ir::schema::ProjectSchemas;

use crate::meta::{Need, Needs, RuleMeta};
use crate::passes::SkipReason;

/// Downstream demand for this model, for `cross.*` rules. Sorted maps
/// so iteration stays deterministic.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DagFacts {
    /// Columns downstream models read from this model, by model name.
    pub downstream_columns: BTreeMap<Arc<str>, Vec<Arc<str>>>,
}

/// Read-only inputs. `available` states what is actually usable, so
/// `Skipped` reasons can name the need; the optional fields stay `None`
/// when their need is absent.
#[derive(Debug, Clone, Copy)]
pub struct Cx<'a> {
    pub available: Needs,
    pub hir: Option<&'a Hir>,
    /// The canonical plan, set by the driver for LIR detect: LIR scalars
    /// and columns are MIR ids.
    pub mir: Option<&'a Mir>,
    pub dialect: Option<Backend>,
    pub mat: Option<Materialization>,
    pub config: Option<&'a ModelConfig>,
    pub schemas: Option<&'a ProjectSchemas>,
    pub phys: Option<&'a PhysicalFacts>,
    pub dag: Option<&'a DagFacts>,
}

impl<'a> Cx<'a> {
    pub fn new(available: Needs) -> Self {
        Self {
            available,
            hir: None,
            mir: None,
            dialect: None,
            mat: None,
            config: None,
            schemas: None,
            phys: None,
            dag: None,
        }
    }

    pub fn has(&self, need: Need) -> bool {
        self.available.contains(need)
    }

    /// Why `meta` cannot run here, naming the first unmet need.
    pub fn skip_reason(&self, meta: &RuleMeta) -> Option<SkipReason> {
        meta.needs
            .unmet(self.available)
            .next()
            .map(SkipReason::Unmet)
    }
}
