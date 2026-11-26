//! Documentation and test coverage of a model's columns: how many of the
//! columns it is known to have carry a `description:` and how many at
//! least one data test. Pure over the context's YAML columns and, when
//! the caller has it, the model's lineage (a produced column the YAML
//! never names counts as neither documented nor tested — the same
//! `missing_columns` the undocumented-columns lint reports).

use serde::Serialize;

use super::super::lint::undocumented::missing_columns;
use crate::context::ModelDetails;
use crate::lineage::ModelLineage;
use crate::DbtContext;

/// One model's figures (the dbt.modelInfo `coverage` shape).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCoverage {
    /// Columns known: declared in the YAML, plus produced-but-undeclared
    /// ones when lineage was available.
    pub columns: usize,
    pub documented: usize,
    pub tested: usize,
    /// The model's own `description:` is non-empty.
    pub described: bool,
}

/// The project-wide summary (the dbt.status `project.coverage` shape).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCoverage {
    pub models: usize,
    pub models_described: usize,
    pub columns: usize,
    pub documented: usize,
    pub tested: usize,
}

pub fn model_coverage(details: &ModelDetails, lineage: Option<&ModelLineage>) -> ModelCoverage {
    let undeclared = lineage.map_or(0, |l| missing_columns(details, l).len());
    ModelCoverage {
        columns: details.columns.len() + undeclared,
        documented: details
            .columns
            .iter()
            .filter(|c| !c.description.trim().is_empty())
            .count(),
        tested: details
            .columns
            .iter()
            .filter(|c| !c.tests.is_empty())
            .count(),
        described: !details.description.trim().is_empty(),
    }
}

/// Every model of the own project, from the YAML alone (no lineage: this
/// summary must stay cheap enough for dbt.status).
pub fn project_coverage(ctx: &DbtContext) -> ProjectCoverage {
    ctx.model_detail_map
        .values()
        .filter(|d| d.project_name == ctx.project.name)
        .map(|d| model_coverage(d, None))
        .fold(ProjectCoverage::default(), |acc, c| ProjectCoverage {
            models: acc.models + 1,
            models_described: acc.models_described + usize::from(c.described),
            columns: acc.columns + c.columns,
            documented: acc.documented + c.documented,
            tested: acc.tested + c.tested,
        })
}

impl ModelCoverage {
    /// The lens title: `3/7 columns documented · 2/7 tested`, with a
    /// `· no description` tail for an undescribed model.
    pub fn title(&self) -> String {
        let mut title = if self.columns == 0 {
            "no columns declared".to_string()
        } else {
            format!(
                "{}/{} columns documented · {}/{} tested",
                self.documented, self.columns, self.tested, self.columns
            )
        };
        if !self.described {
            title.push_str(" · no description");
        }
        title
    }
}

#[cfg(test)]
mod tests {
    use cogitator_schemas::properties::ColumnTest;
    use cogitator_schemas::ColumnEntry;

    use super::*;
    use crate::lineage::ColumnLineage;

    fn column(name: &str, description: &str, tested: bool) -> ColumnEntry {
        ColumnEntry {
            name: name.into(),
            description: description.into(),
            name_pos: Default::default(),
            data_type: None,
            tests: if tested {
                vec![ColumnTest {
                    name: "not_null".into(),
                    strict: true,
                }]
            } else {
                Vec::new()
            },
        }
    }

    fn details(description: &str) -> ModelDetails {
        ModelDetails {
            description: description.into(),
            columns: vec![
                column("a", "the a", true),
                column("b", "  ", true),
                column("c", "the c", false),
            ],
            ..Default::default()
        }
    }

    #[test]
    fn titles() {
        let lineage = ModelLineage {
            columns: ["a", "d", "*"]
                .iter()
                .map(|n| ColumnLineage {
                    name: n.to_string(),
                    sources: Vec::new(),
                    direct: false,
                    ..Default::default()
                })
                .collect(),
            origin: None,
        };
        let cases = [
            (details("x"), None, "2/3 columns documented · 2/3 tested"),
            (
                details("x"),
                Some(&lineage),
                "2/4 columns documented · 2/4 tested",
            ),
            (
                details(""),
                None,
                "2/3 columns documented · 2/3 tested · no description",
            ),
            (
                ModelDetails::default(),
                None,
                "no columns declared · no description",
            ),
        ];
        for (details, lineage, expected) in cases {
            assert_eq!(model_coverage(&details, lineage).title(), expected);
        }
    }

    #[test]
    fn project_sums_own_models() {
        let mut ctx = DbtContext::default();
        ctx.project.name = "p".into();
        let own = ModelDetails {
            project_name: "p".into(),
            ..details("x")
        };
        ctx.model_detail_map.insert("m1".into(), own.clone());
        ctx.model_detail_map.insert(
            "m2".into(),
            ModelDetails {
                description: String::new(),
                ..own
            },
        );
        ctx.model_detail_map.insert("dep".into(), details("x"));
        assert_eq!(
            project_coverage(&ctx),
            ProjectCoverage {
                models: 2,
                models_described: 1,
                columns: 6,
                documented: 4,
                tested: 4,
            }
        );
    }
}
