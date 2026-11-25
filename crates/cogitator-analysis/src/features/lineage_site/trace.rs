//! One model's lineage for the site, from its saved text: the typed route
//! (stars expanded against the project's schemas), and for a template
//! with control flow the compiled SQL the shell found, when fresh.

use crate::artifacts::ArtifactSet;
use crate::features::plan::schemas::SchemaSnapshot;
use crate::features::plan::{CompiledInput, PlanInput};
use crate::lineage::{needs_compiled, typed_column_lineage};
use crate::DbtContext;

use super::Traced;

/// What the shell knows of one model.
pub struct ModelText<'a> {
    pub model: &'a str,
    /// The saved file; `None` when unreadable.
    pub text: Option<&'a str>,
    /// The compiled SQL as the shell read it (`NotChecked` when it did not).
    pub compiled: CompiledInput<'a>,
}

/// The model's lineage, or why there is none.
pub fn trace_model(
    ctx: &DbtContext,
    artifacts: &ArtifactSet,
    project: Option<&SchemaSnapshot>,
    model: ModelText<'_>,
) -> Traced {
    let Some(text) = model.text else {
        return Traced::Unreadable;
    };
    let Ok(parsed) = cogitator_parser::pipeline::parse_document(text, &ctx.dialect) else {
        return Traced::Incomplete;
    };
    let revision = project.map_or(0, |p| p.revision);
    let input = PlanInput::from_context(ctx, text, Some(model.model), artifacts, revision)
        .with_project(project);
    let control_flow = needs_compiled(&parsed);
    let input = if control_flow {
        input.with_compiled(model.compiled)
    } else {
        input
    };
    match typed_column_lineage(&parsed, &input) {
        Some(lineage) => Traced::Lineage(lineage),
        None if control_flow => Traced::ControlFlow,
        None => Traced::Incomplete,
    }
}
