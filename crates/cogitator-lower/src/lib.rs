//! HIR → MIR and MIR → LIR lowering (specs/ir-pipeline/02, 03).
//!
//! Wave 1 stub: both stages decline. Type inference, the lowering rules
//! and the five backends land in `feat/ir-lowering`.

pub mod backends;

use cogitator_ir::hir::Hir;
use cogitator_ir::lir::{Capabilities, Lir, PhysColumn, PhysicalFacts, Pred, Pruning};
use cogitator_ir::mir::Mir;
use cogitator_ir::{Degradation, DegradationKind, Lowered};

/// A dialect's physical knowledge. Implemented by one unit struct per
/// module under `backends/`; `lower` must return a `Lir` that passes
/// `verify_lir` against the input `Mir` and `capabilities()`.
pub trait DialectBackend: Sync {
    fn capabilities(&self) -> &'static Capabilities;
    fn lower(&self, mir: &Mir, phys: &PhysicalFacts) -> Lir;
    fn pruning(&self, pred: &Pred, col: &PhysColumn) -> Pruning;
}

/// Lowers a verified HIR to MIR. Never fails: gaps become `Opaque`
/// nodes with `Degradation`s or a `Declined` result.
pub fn lower_to_mir(_hir: &Hir) -> Lowered<Mir> {
    Lowered::declined(vec![Degradation::new(
        DegradationKind::NotImplemented,
        "HIR to MIR lowering is not implemented yet",
    )])
}

/// Lowers a verified MIR through `backend`.
pub fn lower_to_lir(
    _mir: &Mir,
    _backend: &dyn DialectBackend,
    _phys: &PhysicalFacts,
) -> Lowered<Lir> {
    Lowered::declined(vec![Degradation::new(
        DegradationKind::NotImplemented,
        "MIR to LIR lowering is not implemented yet",
    )])
}

#[cfg(test)]
mod tests {
    use super::*;
    use cogitator_ir::origin::{SourceOrigin, TemplateVariant};
    use cogitator_ir::testkit::{int_col, MirBuilder};
    use cogitator_ir::Fidelity;

    struct NoBackend;

    impl DialectBackend for NoBackend {
        fn capabilities(&self) -> &'static Capabilities {
            unreachable!("the stubs never consult the backend")
        }
        fn lower(&self, _mir: &Mir, _phys: &PhysicalFacts) -> Lir {
            unreachable!("the stubs never consult the backend")
        }
        fn pruning(&self, _pred: &Pred, _col: &PhysColumn) -> Pruning {
            Pruning::Unknown
        }
    }

    #[test]
    fn stubs_decline_without_panicking() {
        let hir = Hir::new(SourceOrigin::Template {
            variant: TemplateVariant::Plain,
        });
        assert_eq!(lower_to_mir(&hir).fidelity, Fidelity::Declined);

        let mut b = MirBuilder::new();
        let scan = b.scan("t", &[("a", int_col())]);
        let mir = b.finish(scan);
        let lir = lower_to_lir(&mir, &NoBackend, &PhysicalFacts::default());
        assert_eq!(lir.fidelity, Fidelity::Declined);
    }
}
