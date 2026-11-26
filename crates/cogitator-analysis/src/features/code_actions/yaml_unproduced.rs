//! Tests on a column the model does not produce: a properties entry
//! whose column carries `data_tests:` (or `tests:`) though the model's
//! SQL yields no such column — the test fails at `dbt test` with a
//! warehouse error. Only where lineage is certain of the output (no
//! `*` of an unknown upstream), and not under an enforced contract,
//! whose own drift check names the same columns. A Warning, with the
//! fixes as the diagnostic's data: rename to the closest produced
//! column, or remove the column's entry.

use cogitator_error::guidance::{closest_matches, Guidance};
use cogitator_lsp::{
    diagnostic_severity, Diagnostic, DiagnosticData, FixKind, MultiEditFix, Position, Range,
    TextEdit,
};

use super::yaml_text::{entry_at, lines_range};
use crate::context::ModelDetails;
use crate::lineage::ModelLineage;

pub const CODE: &str = "yaml-unproduced-column";

/// One warning per tested column of `model`'s entry in `yaml_text` that
/// its lineage does not produce.
pub fn unproduced_column_diagnostics(
    yaml_text: &str,
    model: &str,
    details: &ModelDetails,
    lineage: &ModelLineage,
) -> Vec<Diagnostic> {
    let certain =
        !lineage.columns.is_empty() && !lineage.columns.iter().any(|c| c.name.contains('*'));
    if !certain || details.contract_enforced {
        return Vec::new();
    }
    let produced: Vec<&str> = lineage.columns.iter().map(|c| c.name.as_str()).collect();
    let lines: Vec<&str> = yaml_text.lines().collect();
    let mut out = Vec::new();
    for column in details.columns.iter().filter(|c| !c.tests.is_empty()) {
        if produced
            .iter()
            .any(|p| p.eq_ignore_ascii_case(&column.name))
        {
            continue;
        }
        let Some(range) = name_range(&lines, column.name_pos, &column.name) else {
            continue; // not written in this file (catalog-filled)
        };
        let suggestions = closest_matches(&column.name, produced.iter().copied());
        let mut fixes: Vec<MultiEditFix> = suggestions
            .iter()
            .map(|s| MultiEditFix {
                title: format!("Rename column '{}' to '{s}'", column.name),
                edits: vec![TextEdit {
                    range,
                    new_text: s.clone(),
                }],
                preserving: false,
            })
            .collect();
        if let Some(entry) = entry_at(&lines, range.start.line as usize) {
            fixes.push(MultiEditFix {
                title: format!("Remove column '{}' from '{model}'", column.name),
                edits: vec![TextEdit {
                    range: lines_range(yaml_text, entry.dash_line, entry.end),
                    new_text: String::new(),
                }],
                preserving: false,
            });
        }
        let guidance = Guidance::new(format!(
            "'{model}' does not produce column '{}', which its tests read{}",
            column.name,
            lineage.origin_note()
        ))
        .suggest(suggestions.clone())
        .fix("rename it to a column the model selects, or remove the entry.");
        let mut diagnostic = Diagnostic::plain(
            range,
            guidance.to_diagnostic(),
            diagnostic_severity::WARNING,
            CODE,
            "cogitator",
        );
        diagnostic.data = Some(DiagnosticData {
            fix: FixKind::MultiEdit { fixes },
            token_range: range,
            suggestions,
            subject: None,
        });
        out.push(diagnostic);
    }
    out
}

/// The range of `name` written at `pos` (inside quotes when quoted);
/// None when the text there is not that name.
fn name_range(lines: &[&str], pos: Position, name: &str) -> Option<Range> {
    let line = lines.get(pos.line as usize)?;
    let mut units = 0usize;
    let byte = line
        .char_indices()
        .find(|(_, c)| {
            let here = units;
            units += c.len_utf16();
            here >= pos.character as usize
        })
        .map(|(i, _)| i)?;
    let rest = &line[byte..];
    let quote = usize::from(rest.starts_with(['"', '\'']));
    if !rest[quote..].starts_with(name) {
        return None;
    }
    let start = pos.character + quote as u32;
    let end = start + name.encode_utf16().count() as u32;
    Some(Range {
        start: Position {
            line: pos.line,
            character: start,
        },
        end: Position {
            line: pos.line,
            character: end,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lineage::ColumnLineage;
    use cogitator_schemas::properties::ColumnTest;
    use cogitator_schemas::ColumnEntry;
    use rstest::rstest;

    const YAML: &str = "models:\n  - name: customers\n    columns:\n      - name: customer_id\n        data_tests:\n          - unique\n      - name: frist_name\n        data_tests:\n          - not_null\n      - name: untested\n";

    fn column(name: &str, line: u32, tested: bool) -> ColumnEntry {
        ColumnEntry {
            name: name.to_string(),
            description: String::new(),
            name_pos: Position {
                line,
                character: 14,
            },
            data_type: None,
            tests: if tested {
                vec![ColumnTest {
                    name: "not_null".into(),
                    strict: true,
                }]
            } else {
                Vec::new()
            },
            constraints: Vec::new(),
        }
    }

    fn details(contract: bool) -> ModelDetails {
        ModelDetails {
            columns: vec![
                column("customer_id", 3, true),
                column("frist_name", 6, true),
                column("untested", 9, false),
            ],
            contract_enforced: contract,
            ..Default::default()
        }
    }

    fn lineage(names: &[&str]) -> ModelLineage {
        ModelLineage {
            columns: names
                .iter()
                .map(|n| ColumnLineage {
                    name: n.to_string(),
                    sources: Vec::new(),
                    direct: false,
                })
                .collect(),
            origin: None,
        }
    }

    #[rstest]
    #[case::misspelled(&["customer_id", "first_name"], false, 1)]
    #[case::all_produced(&["customer_id", "frist_name"], false, 0)]
    #[case::star_is_uncertain(&["customer_id", "*"], false, 0)]
    #[case::unknown_output(&[], false, 0)]
    #[case::contract_has_its_own_check(&["customer_id"], true, 0)]
    #[case::case_insensitive(&["CUSTOMER_ID", "FRIST_NAME"], false, 0)]
    fn only_tested_unproduced_columns_warn(
        #[case] produced: &[&str],
        #[case] contract: bool,
        #[case] expected: usize,
    ) {
        let got = unproduced_column_diagnostics(
            YAML,
            "customers",
            &details(contract),
            &lineage(produced),
        );
        assert_eq!(got.len(), expected, "{got:?}");
    }

    #[test]
    fn the_fixes_rename_or_remove() {
        let got = unproduced_column_diagnostics(
            YAML,
            "customers",
            &details(false),
            &lineage(&["customer_id", "first_name"]),
        );
        let d = &got[0];
        assert_eq!(d.severity, diagnostic_severity::WARNING);
        assert_eq!(
            (
                d.range.start.line,
                d.range.start.character,
                d.range.end.character
            ),
            (6, 14, 24)
        );
        let Some(FixKind::MultiEdit { fixes }) = d.data.as_ref().map(|x| &x.fix) else {
            panic!("no fixes");
        };
        assert_eq!(fixes[0].title, "Rename column 'frist_name' to 'first_name'");
        assert_eq!(fixes[0].edits[0].new_text, "first_name");
        let remove = fixes.last().unwrap();
        assert_eq!(remove.title, "Remove column 'frist_name' from 'customers'");
        assert_eq!(remove.edits[0].range.start.line, 6);
        assert_eq!(remove.edits[0].range.end.line, 9);
    }

    #[test]
    fn no_close_name_leaves_only_removal() {
        let got = unproduced_column_diagnostics(
            YAML,
            "customers",
            &details(false),
            &lineage(&["customer_id", "zzz"]),
        );
        let Some(FixKind::MultiEdit { fixes }) = got[0].data.as_ref().map(|x| &x.fix) else {
            panic!("no fixes");
        };
        assert_eq!(fixes.len(), 1);
        assert!(fixes[0].title.starts_with("Remove"));
    }

    #[test]
    fn a_column_written_elsewhere_is_skipped() {
        let mut d = details(false);
        d.columns[1].name_pos = Position {
            line: 0,
            character: 0,
        };
        let got = unproduced_column_diagnostics(YAML, "customers", &d, &lineage(&["customer_id"]));
        assert!(got.is_empty());
    }
}
