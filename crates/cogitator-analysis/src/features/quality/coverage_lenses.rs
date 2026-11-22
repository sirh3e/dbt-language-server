//! The coverage lenses: "3/7 columns documented · 2/7 tested" on a
//! model's SQL file and on its properties YAML entry, and a lens on each
//! YAML column that lacks a description or a test, saying which.

use cogitator_lsp::{CodeLens, Command, Range};

use super::coverage::model_coverage;
use crate::context::ModelDetails;
use crate::features::model_name_from_uri;
use crate::lineage::ModelLineage;
use crate::DbtContext;

/// On a model's SQL file: its coverage, counting produced-but-undeclared
/// columns when `lineage` is known. Nothing for a model without a YAML
/// entry (the "Extract schema YAML" lens covers that case).
pub fn model_coverage_lens(
    ctx: &DbtContext,
    uri: &str,
    lineage: Option<&ModelLineage>,
) -> Option<CodeLens> {
    let model = model_name_from_uri(uri)?;
    let details = ctx.model_detail_map.get(&model)?;
    details.schema_uri.as_ref()?;
    Some(info_lens(
        Range::default(),
        model_coverage(details, lineage).title(),
    ))
}

/// In a properties YAML: per model entry of this file, its coverage at
/// the entry's name, and one lens per column missing a description or a
/// test.
pub fn yaml_coverage_lenses(ctx: &DbtContext, uri: &str) -> Vec<CodeLens> {
    let path = cogitator_lsp::uri_to_fs_path(uri).unwrap_or_else(|| uri.to_string());
    let path = std::path::Path::new(&path);
    let mut entries: Vec<&ModelDetails> = ctx
        .model_detail_map
        .values()
        // Path equality compares components, so `/` and `\` match
        .filter(|d| d.schema_uri.as_deref() == Some(path))
        .collect();
    entries.sort_by_key(|d| (d.schema_range.start.line, d.schema_range.start.character));
    entries
        .into_iter()
        .flat_map(|details| {
            let head = info_lens(details.schema_range, model_coverage(details, None).title());
            std::iter::once(head).chain(details.columns.iter().filter_map(|column| {
                let gap = column_gap(
                    !column.description.trim().is_empty(),
                    !column.tests.is_empty(),
                )?;
                Some(info_lens(
                    Range::zero_width(column.name_pos),
                    gap.to_string(),
                ))
            }))
        })
        .collect()
}

/// What a column lacks, as its lens title; None when it lacks nothing.
pub(crate) fn column_gap(documented: bool, tested: bool) -> Option<&'static str> {
    match (documented, tested) {
        (true, true) => None,
        (true, false) => Some("no tests"),
        (false, true) => Some("no description"),
        (false, false) => Some("no description · no tests"),
    }
}

fn info_lens(range: Range, title: String) -> CodeLens {
    CodeLens {
        range,
        command: Command {
            title,
            command: String::new(),
            arguments: Vec::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_gap_names_what_is_missing() {
        let cases = [
            (true, true, None),
            (true, false, Some("no tests")),
            (false, true, Some("no description")),
            (false, false, Some("no description · no tests")),
        ];
        for (documented, tested, expected) in cases {
            assert_eq!(column_gap(documented, tested), expected);
        }
    }
}
