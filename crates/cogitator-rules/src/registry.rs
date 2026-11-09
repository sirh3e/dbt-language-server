//! The static, ordered rule list. Order is run order within a phase:
//! canonicalisers first, then detectors by level.

use cogitator_ir::hir::Hir;
use cogitator_ir::lir::Lir;
use cogitator_ir::mir::Mir;

use crate::canon::{predicates, required_columns, simplify};
use crate::meta::RuleMeta;
use crate::rule::{Canonicalize, Detect};
use crate::rules::{cross, filter, incr, join, model, set};

/// One rule's meta and the phases it implements.
pub struct Registered {
    pub meta: &'static RuleMeta,
    pub canon: Option<&'static dyn Canonicalize>,
    pub hir: Option<&'static dyn Detect<Hir>>,
    pub mir: Option<&'static dyn Detect<Mir>>,
    pub lir: Option<&'static dyn Detect<Lir>>,
}

const EMPTY: Registered = Registered {
    meta: &simplify::META,
    canon: None,
    hir: None,
    mir: None,
    lir: None,
};

pub static REGISTRY: &[Registered] = &[
    Registered {
        meta: &simplify::META,
        canon: Some(&simplify::Simplify),
        ..EMPTY
    },
    Registered {
        meta: &predicates::META,
        canon: Some(&predicates::Predicates),
        ..EMPTY
    },
    Registered {
        meta: &required_columns::META,
        canon: Some(&required_columns::RequiredColumns),
        ..EMPTY
    },
    Registered {
        meta: &model::unused_cte::META,
        hir: Some(&model::unused_cte::UnusedCte),
        ..EMPTY
    },
    Registered {
        meta: &incr::missing_guard::META,
        hir: Some(&incr::missing_guard::MissingGuard),
        ..EMPTY
    },
    Registered {
        meta: &incr::subquery_watermark::META,
        hir: Some(&incr::subquery_watermark::SubqueryWatermark),
        lir: Some(&incr::subquery_watermark::SubqueryWatermark),
        ..EMPTY
    },
    Registered {
        meta: &join::left_nullified::META,
        mir: Some(&join::left_nullified::LeftNullified),
        ..EMPTY
    },
    Registered {
        meta: &join::cartesian::META,
        mir: Some(&join::cartesian::Cartesian),
        ..EMPTY
    },
    Registered {
        meta: &join::distinct_fanout::META,
        mir: Some(&join::distinct_fanout::DistinctFanout),
        ..EMPTY
    },
    Registered {
        meta: &cross::select_star_chain::META,
        mir: Some(&cross::select_star_chain::SelectStarChain),
        ..EMPTY
    },
    Registered {
        meta: &set::union_distinct::META,
        mir: Some(&set::union_distinct::UnionDistinct),
        ..EMPTY
    },
    Registered {
        meta: &filter::non_sargable::META,
        lir: Some(&filter::non_sargable::NonSargable),
        ..EMPTY
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use cogitator_ir::Level;

    const TRANCHE_ONE: [&str; 12] = [
        "canon.simplify",
        "canon.predicates",
        "canon.required_columns",
        "join.left_nullified",
        "join.cartesian",
        "join.distinct_fanout",
        "model.unused_cte",
        "cross.select_star_chain",
        "filter.non_sargable",
        "incr.missing_guard",
        "incr.subquery_watermark",
        "set.union_distinct",
    ];

    #[test]
    fn registry_covers_tranche_one_exactly() {
        let mut ids: Vec<&str> = REGISTRY.iter().map(|r| r.meta.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), REGISTRY.len(), "duplicate rule id");
        let mut expected = TRANCHE_ONE.to_vec();
        expected.sort_unstable();
        assert_eq!(ids, expected);
    }

    #[test]
    fn each_entry_implements_exactly_its_declared_levels() {
        for rule in REGISTRY {
            let id = rule.meta.id.as_str();
            let canon = rule.meta.class == crate::meta::RuleClass::Canon;
            assert_eq!(rule.canon.is_some(), canon, "{id}");
            if canon {
                continue;
            }
            assert_eq!(
                rule.hir.is_some(),
                rule.meta.level.runs_at(Level::Hir),
                "{id}"
            );
            assert_eq!(
                rule.mir.is_some(),
                rule.meta.level.runs_at(Level::Mir),
                "{id}"
            );
            assert_eq!(
                rule.lir.is_some(),
                rule.meta.level.runs_at(Level::Lir),
                "{id}"
            );
        }
    }

    #[test]
    fn canonicalisers_run_first() {
        let first_advice = REGISTRY
            .iter()
            .position(|r| r.meta.class != crate::meta::RuleClass::Canon)
            .expect("advice rules exist");
        assert_eq!(first_advice, 3);
    }
}
