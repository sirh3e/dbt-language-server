//! Column references: a documented model column's YAML entry, its words
//! in the model's own SQL, and its words in the direct downstream models
//! whose column lineage reads it — as far as the template lineage knows:
//! a dependent the lineage declines (Jinja control blocks) is left out
//! rather than matched by name alone.

use cogitator_lsp::Location;
use cogitator_parser::{DocumentParser, JinjaSqlParser};

use crate::lineage::{column_lineage, SourceRef};

use super::super::{file_uri, DbtContext};
use super::files::{node_files, Reader};
use super::occur::{name_span, word_uses};

pub(crate) fn column_references(
    ctx: &DbtContext,
    reader: &Reader<'_>,
    model: &str,
    column: &str,
    include_declaration: bool,
) -> Vec<Location> {
    let Some(details) = ctx.model_detail_map.get(model) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if include_declaration {
        let entry = details.columns.iter().find(|c| c.name == column);
        if let (Some(schema), Some(entry)) = (&details.schema_uri, entry) {
            let range = reader
                .read(schema)
                .and_then(|text| name_span(&text, entry.name_pos, &entry.name))
                .unwrap_or_else(|| cogitator_lsp::Range::zero_width(entry.name_pos));
            out.push(Location {
                uri: file_uri(schema),
                range,
            });
        }
    }
    let locate = |path: &std::path::Path,
                  reads: &dyn Fn(&cogitator_parser::ParsedDocument) -> bool| {
        let text = reader.read(path)?;
        let parsed = JinjaSqlParser.parse(&text, &ctx.dialect).ok()?;
        reads(&parsed).then(|| {
            word_uses(&parsed, column)
                .into_iter()
                .map(|range| Location {
                    uri: file_uri(path),
                    range,
                })
                .collect::<Vec<_>>()
        })
    };
    if !details.uri.as_os_str().is_empty() {
        out.extend(locate(&details.uri, &|_| true).unwrap_or_default());
    }
    for path in node_files(ctx.graph.downstream_of(model)) {
        let reads = |parsed: &cogitator_parser::ParsedDocument| {
            column_lineage(parsed, &ctx.dialect).is_some_and(|lineage| {
                lineage
                    .columns
                    .iter()
                    .any(|c| c.sources.iter().any(|s| reads_column(s, model, column)))
            })
        };
        out.extend(locate(&path, &reads).unwrap_or_default());
    }
    out
}

fn reads_column(source: &SourceRef, model: &str, column: &str) -> bool {
    matches!(source, SourceRef::Model { name, column: c } if name == model && c.eq_ignore_ascii_case(column))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lineage_sources_match_by_model_and_column() {
        let model = |name: &str, column: &str| SourceRef::Model {
            name: name.into(),
            column: column.into(),
        };
        for (source, expected) in [
            (model("orders", "amount"), true),
            (model("orders", "AMOUNT"), true),
            (model("orders", "status"), false),
            (model("customers", "amount"), false),
            (
                SourceRef::Unknown {
                    name: "orders".into(),
                    column: "amount".into(),
                },
                false,
            ),
        ] {
            assert_eq!(
                reads_column(&source, "orders", "amount"),
                expected,
                "{source:?}"
            );
        }
    }
}
