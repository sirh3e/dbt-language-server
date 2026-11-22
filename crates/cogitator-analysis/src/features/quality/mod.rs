//! Quality lenses: documentation/test coverage of models and columns,
//! and "Run this test" entry points on single tests.

pub mod coverage;
mod coverage_lenses;
mod test_lenses;
pub mod test_names;

pub use coverage::{model_coverage, project_coverage, ModelCoverage, ProjectCoverage};
pub use coverage_lenses::{model_coverage_lens, yaml_coverage_lenses};
pub use test_lenses::{singular_test_lens, yaml_test_lenses, yaml_tests, TestTarget};

use cogitator_lsp::CodeLens;

use crate::lineage::ModelLineage;
use crate::DbtContext;

/// Every quality lens of one document. `text` is the document's text
/// (the open buffer, else the file), needed for a properties YAML;
/// `lineage` the model's, when the caller has it.
pub fn quality_lenses(
    ctx: &DbtContext,
    uri: &str,
    text: Option<&str>,
    lineage: Option<&ModelLineage>,
) -> Vec<CodeLens> {
    if uri.ends_with(".yml") || uri.ends_with(".yaml") {
        let mut lenses = yaml_coverage_lenses(ctx, uri);
        if let Some(text) = text {
            lenses.extend(yaml_test_lenses(uri, text));
        }
        lenses.sort_by_key(|l| (l.range.start.line, l.range.start.character));
        return lenses;
    }
    model_coverage_lens(ctx, uri, lineage)
        .into_iter()
        .chain(singular_test_lens(ctx, uri))
        .collect()
}
