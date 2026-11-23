//! What a removed or retyped column breaks: its own tests, the
//! downstream columns whose lineage reads it (dbt.impact's column
//! closure), and whether those are tested, contracted or public.

use std::collections::HashMap;

use serde::Serialize;

use super::SchemaDiff;
use crate::features::DbtContext;
use crate::lineage::ModelLineage;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ChangeKind {
    Removed,
    Retyped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Error,
    Warning,
    Information,
}

/// A downstream column fed by the changed column.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownstreamUse {
    pub model: String,
    pub column: String,
    /// Column tests on the downstream column.
    pub tests: Vec<String>,
    /// `contract.enforced` on the downstream model.
    pub contracted: bool,
    /// `access: public` on the downstream model.
    pub public: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnImpact {
    pub column: String,
    pub change: ChangeKind,
    pub severity: Severity,
    /// Column tests on the changed column itself.
    pub tests: Vec<String>,
    pub downstream: Vec<DownstreamUse>,
}

pub(super) fn impacts(
    ctx: &DbtContext,
    model: &str,
    diff: &SchemaDiff,
    lineages: &HashMap<String, ModelLineage>,
) -> Vec<ColumnImpact> {
    let public = |name: &str| {
        ctx.model_detail_map
            .get(name)
            .is_some_and(|d| d.access.as_deref() == Some("public"))
    };
    let model_public = public(model);
    let has_exposures = !crate::features::impact(ctx, model, None, lineages)
        .exposures
        .is_empty();
    let changed = diff
        .removed
        .iter()
        .map(|r| (r.name.as_str(), ChangeKind::Removed))
        .chain(
            diff.retyped
                .iter()
                .map(|t| (t.name.as_str(), ChangeKind::Retyped)),
        );
    changed
        .map(|(column, change)| {
            let downstream: Vec<DownstreamUse> =
                crate::features::impact(ctx, model, Some(column), lineages)
                    .column_hits
                    .into_iter()
                    .map(|hit| DownstreamUse {
                        tests: column_tests(ctx, &hit.model, &hit.column),
                        contracted: ctx
                            .model_detail_map
                            .get(&hit.model)
                            .is_some_and(|d| d.contract_enforced),
                        public: public(&hit.model),
                        model: hit.model,
                        column: hit.column,
                    })
                    .collect();
            let tests = column_tests(ctx, model, column);
            ColumnImpact {
                severity: severity(change, &tests, &downstream, model_public || has_exposures),
                column: column.to_string(),
                change,
                tests,
                downstream,
            }
        })
        .collect()
}

/// Error only for a removed column a test or a contracted downstream
/// relies on; warning when anything reads it (downstream columns,
/// external consumers of a public model, exposures); information else.
pub(super) fn severity(
    change: ChangeKind,
    tests: &[String],
    downstream: &[DownstreamUse],
    consumed_outside: bool,
) -> Severity {
    let relied_on = !tests.is_empty()
        || downstream
            .iter()
            .any(|d| d.contracted || !d.tests.is_empty());
    if change == ChangeKind::Removed && relied_on {
        return Severity::Error;
    }
    if relied_on || !downstream.is_empty() || consumed_outside {
        return Severity::Warning;
    }
    Severity::Information
}

fn column_tests(ctx: &DbtContext, model: &str, column: &str) -> Vec<String> {
    ctx.model_detail_map
        .get(model)
        .and_then(|d| {
            d.columns
                .iter()
                .find(|c| c.name.eq_ignore_ascii_case(column))
        })
        .map(|c| c.tests.iter().map(|t| t.name.clone()).collect())
        .unwrap_or_default()
}
