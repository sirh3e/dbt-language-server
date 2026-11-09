//! Builds HIR from dbt SQL+Jinja (specs/ir-pipeline/01). The only crate
//! in the pipeline that sees sqlparser or `minijinja::machinery`.
//!
//! Wave 1 stub: `build_hir` declines. Masking, statement segmentation,
//! the binder and the `is_incremental()` variants land in `feat/ir-hir`
//! and the compiled-SQL fallback in `compiled/` (`feat/ir-compiled`).

use std::sync::Arc;

use cogitator_dialects::Dialect;
use cogitator_ir::common::ModelConfig;
use cogitator_ir::hir::Hir;
use cogitator_ir::origin::SourceOrigin;
use cogitator_ir::schema::ProjectSchemas;
use cogitator_ir::{Degradation, DegradationKind, Lowered};

/// One model's text and the facts the front end needs beside it.
#[derive(Debug, Clone)]
pub struct HirInput<'a> {
    /// The text spans will index: the live template or compiled SQL.
    pub text: &'a str,
    pub dialect: &'a Dialect,
    pub origin: SourceOrigin,
    pub config: ModelConfig,
}

/// Name-resolution context for relations outside the model.
#[derive(Debug, Clone, Default)]
pub struct SchemaEnv {
    pub schemas: Arc<ProjectSchemas>,
    /// The model being built, for `{{ this }}` and self references.
    pub model: Option<Arc<str>>,
}

/// Builds and binds the HIR for `input`. Never fails: gaps surface as
/// `Degradation`s or a `Declined` result carrying guidance.
pub fn build_hir(_input: &HirInput<'_>, _env: &SchemaEnv) -> Lowered<Hir> {
    Lowered::declined(vec![Degradation::new(
        DegradationKind::NotImplemented,
        "the HIR front end is not implemented yet",
    )])
}

#[cfg(test)]
mod tests {
    use super::*;
    use cogitator_ir::origin::TemplateVariant;
    use cogitator_ir::Fidelity;

    #[test]
    fn stub_declines_without_panicking() {
        let dialect = Dialect::new("duckdb");
        let input = HirInput {
            text: "select 1",
            dialect: &dialect,
            origin: SourceOrigin::Template {
                variant: TemplateVariant::Plain,
            },
            config: ModelConfig::default(),
        };
        let lowered = build_hir(&input, &SchemaEnv::default());
        assert_eq!(lowered.fidelity, Fidelity::Declined);
        assert!(lowered.value.is_none());
    }
}
